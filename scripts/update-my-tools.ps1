# Background update worker embedded in my-tools-agent.exe.
# For private GitHub releases it uses the locally authenticated GitHub CLI.
param(
    [Parameter(Mandatory = $true)][string]$Repo,
    [Parameter(Mandatory = $true)][string]$CurrentVersion,
    [Parameter(Mandatory = $true)][string]$InstallDir,
    [int]$AgentPid = 0,
    [switch]$Interactive
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$dataDir = Join-Path $env:LOCALAPPDATA 'MyTools'
$log = Join-Path $dataDir 'update.log'
$stage = Join-Path $dataDir 'update-staging'
$package = 'MyTools-windows-x64.zip'

function Log([string]$message) {
    $timestamp = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss')
    Add-Content -LiteralPath $log -Value "$timestamp $message"
    if ($Interactive) { Write-Host $message }
}

function Update {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "GitHub CLI (gh) is required for updates from this private repo. Install gh, then run gh auth login."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $InstallDir 'my-tools-agent.exe'))) {
        throw "My Tools isn't installed in $InstallDir. Install the release package first."
    }
    $tag = (& gh release view --repo $Repo --json tagName --jq '.tagName' 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "Cannot check GitHub releases for $Repo. Confirm gh auth status. $tag" }
    $tag = ($tag | Out-String).Trim()
    if ($tag -notmatch '^v(\d+)\.(\d+)\.(\d+)$') {
        throw "Unexpected GitHub release tag '$tag'. Expected vMAJOR.MINOR.PATCH."
    }
    $remote = [version]$tag.Substring(1)
    $current = [version]$CurrentVersion
    if ($remote -le $current) {
        Log "Up to date ($CurrentVersion)."
        return
    }
    # Don't interrupt work in the larger GPUI hub: retry on the next check.
    if (Get-Process -Name 'my-tools' -ErrorAction SilentlyContinue) {
        Log "Version $tag is available; close the My Tools desktop window to allow automatic installation."
        return
    }

    Log "Downloading $tag from $Repo..."
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    & gh release download $tag --repo $Repo --dir $stage --pattern $package --pattern 'SHA256SUMS.txt' --clobber
    if ($LASTEXITCODE -ne 0) { throw "Could not download the GitHub release files." }
    $zip = Join-Path $stage $package
    $hashFile = Join-Path $stage 'SHA256SUMS.txt'
    if (-not (Test-Path -LiteralPath $zip) -or -not (Test-Path -LiteralPath $hashFile)) {
        throw "Release is incomplete: missing zip or checksum file."
    }
    $expectedLine = Get-Content -LiteralPath $hashFile | Where-Object { $_ -match "^[a-fA-F0-9]{64}\s+MyTools-windows-x64\.zip$" } | Select-Object -First 1
    if (-not $expectedLine) { throw "Missing SHA-256 entry for the Windows release." }
    $expected = $expectedLine.Substring(0, 64).ToUpperInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash
    if ($actual -ne $expected) { throw "SHA-256 mismatch. Refusing to install tampered release." }
    $extract = Join-Path $stage 'extracted'
    Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force
    foreach ($name in @('my-tools.exe', 'my-tools-agent.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $extract $name))) {
            throw "Release archive is missing $name."
        }
    }
    # The helper copies files only after the agent has shut down gracefully,
    # allowing Prompt Notes to save its current draft before update.
    if ($AgentPid -le 0) { throw "Missing agent process identifier; update is not safe." }
    Log "Verified $tag. Applying update..."
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MyToolsNative {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);
}
'@
    $hwnd = [MyToolsNative]::FindWindow('MyToolsAgentWin32', $null)
    if ($hwnd -eq [IntPtr]::Zero) { throw 'Cannot find My Tools agent tray window.' }
    if (-not [MyToolsNative]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
        throw 'Could not request graceful agent shutdown.'
    }
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Process -Id $AgentPid -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    if (Get-Process -Id $AgentPid -ErrorAction SilentlyContinue) {
        throw "Agent did not exit gracefully; update cancelled rather than terminating your work."
    }
    $backup = Join-Path $stage 'backup'
    New-Item -ItemType Directory -Path $backup | Out-Null
    foreach ($name in @('my-tools.exe', 'my-tools-agent.exe')) {
        $destination = Join-Path $InstallDir $name
        if (Test-Path -LiteralPath $destination) {
            Copy-Item -LiteralPath $destination -Destination (Join-Path $backup $name) -Force
        }
    }
    try {
        foreach ($name in @('my-tools.exe', 'my-tools-agent.exe')) {
            Copy-Item -LiteralPath (Join-Path $extract $name) -Destination (Join-Path $InstallDir $name) -Force
        }
    } catch {
        foreach ($name in @('my-tools.exe', 'my-tools-agent.exe')) {
            $old = Join-Path $backup $name
            if (Test-Path -LiteralPath $old) {
                Copy-Item -LiteralPath $old -Destination (Join-Path $InstallDir $name) -Force
            }
        }
        throw "Couldn't replace both executables. Previous binaries restored: $_"
    }
    Start-Process -FilePath (Join-Path $InstallDir 'my-tools-agent.exe') -WorkingDirectory $InstallDir
    Log "Updated My Tools to $tag and restarted the background agent."
}

New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
$mutex = [System.Threading.Mutex]::new($false, 'Local\MyToolsGithubUpdate')
$hasLock = $false
try {
    $hasLock = $mutex.WaitOne(0)
    if (-not $hasLock) {
        Log 'An update check is already running.'
        return
    }
    Update
} catch {
    Log "Update failed: $_"
    if ($Interactive) { Write-Host "Update failed: $_" -ForegroundColor Red }
    exit 1
} finally {
    # Staging files are transient; pairing tokens and note drafts aren't touched.
    if ($hasLock -and (Test-Path -LiteralPath $stage)) {
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
    }
    if ($hasLock) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
