# Embedded in my-tools.exe. This helper runs only during GitHub update checks.
# The distributable contains one executable, not a separate updater script.
param(
    [Parameter(Mandatory=$true)][string]$Repo,
    [Parameter(Mandatory=$true)][string]$CurrentVersion,
    [Parameter(Mandatory=$true)][string]$InstallDir,
    [int]$AgentPid = 0,
    [switch]$Interactive
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$dataDir = Join-Path $env:LOCALAPPDATA 'MyTools'
$stage = Join-Path $dataDir 'update-staging'
$logFile = Join-Path $dataDir 'update.log'
$binaryName = 'my-tools.exe'

function Log([string]$message) {
    Add-Content -LiteralPath $logFile -Value "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $message"
    if ($Interactive) { Write-Host $message }
}

function RunUpdate {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw 'Install GitHub CLI and run gh auth login to update this private app.'
    }
    $installedExe = Join-Path $InstallDir $binaryName
    if (-not (Test-Path -LiteralPath $installedExe)) {
        throw 'The installed my-tools.exe was not found.'
    }
    $tag = (& gh release view --repo $Repo --json tagName --jq '.tagName' 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw "Cannot read private release: $tag" }
    if ($tag -notmatch '^v(\d+)\.(\d+)\.(\d+)$') { throw "Unexpected version: $tag" }
    if ([version]$tag.Substring(1) -le [version]$CurrentVersion) {
        Log "Up to date ($CurrentVersion)."
        return
    }

    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MyToolsNative {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);
}
'@
    $dashboard = [MyToolsNative]::FindWindow($null, 'My Tools')
    if ($dashboard -ne [IntPtr]::Zero -and [MyToolsNative]::IsWindowVisible($dashboard)) {
        Log "Version $tag available. Update deferred until My Tools is hidden to the tray."
        return
    }

    Log "Downloading $tag..."
    if (Test-Path $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    & gh release download $tag --repo $Repo --dir $stage --pattern $binaryName --pattern 'SHA256SUMS.txt' --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Release download failed.' }
    $downloaded = Join-Path $stage $binaryName
    $hashFile = Join-Path $stage 'SHA256SUMS.txt'
    if (-not (Test-Path $downloaded) -or -not (Test-Path $hashFile)) {
        throw 'Release executable or checksum is missing.'
    }
    $expectedLine = Get-Content $hashFile | Where-Object { $_ -match '^[a-fA-F0-9]{64}\s+my-tools\.exe$' } | Select-Object -First 1
    if (-not $expectedLine) { throw 'Could not find the SHA256SUMS.txt entry for my-tools.exe.' }
    $expected = $expectedLine.Substring(0,64).ToUpperInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $downloaded).Hash
    if ($expected -ne $actual) { throw 'SHA-256 verification failed: refusing the downloaded executable.' }

    if ($AgentPid -le 0) { throw 'Missing running process ID.' }
    $tray = [MyToolsNative]::FindWindow('MyToolsAgentWin32', $null)
    if ($tray -eq [IntPtr]::Zero) { throw 'My Tools tray was not found; update deferred.' }
    # WM_APP + 43 requests a graceful shutdown of the entire My Tools process.
    if (-not [MyToolsNative]::PostMessage($tray, 0x802B, [IntPtr]::Zero, [IntPtr]::Zero)) {
        throw 'Could not request a graceful My Tools shutdown.'
    }
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Process -Id $AgentPid -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    if (Get-Process -Id $AgentPid -ErrorAction SilentlyContinue) {
        throw 'My Tools did not exit cleanly; keeping the installed version.'
    }

    $backup = Join-Path $stage 'my-tools-backup.exe'
    Copy-Item -LiteralPath $installedExe -Destination $backup -Force
    try {
        Copy-Item -LiteralPath $downloaded -Destination $installedExe -Force
    } catch {
        Copy-Item -LiteralPath $backup -Destination $installedExe -Force
        throw "Update failed. Previous version restored: $_"
    }
    Start-Process -FilePath $installedExe -ArgumentList '--minimized' -WorkingDirectory $InstallDir
    Log "Installed $tag and restarted My Tools in the tray."
}

New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
$mutex = [System.Threading.Mutex]::new($false, 'Local\MyToolsGithubUpdate')
$locked = $false
try {
    $locked = $mutex.WaitOne(0)
    if ($locked) { RunUpdate }
} catch {
    Log "Update failed: $_"
    if ($Interactive) { Write-Host "Update failed: $_" -ForegroundColor Red }
    exit 1
} finally {
    if ($locked -and (Test-Path $stage)) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue }
    if ($locked) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
