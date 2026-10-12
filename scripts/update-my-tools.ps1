# Embedded in my-tools.exe. Public GitHub release updater, no gh or auth needed.
# Distributed as part of the EXE, and written to a temp file only when checking.
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
$expectedRepo = 'LETHEVIET/my-tools'

function Log([string]$message) {
    Add-Content -LiteralPath $logFile -Value "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $message"
    if ($Interactive) { Write-Host $message }
}

function RunUpdate {
    if ($Repo -ne $expectedRepo) { throw 'Unrecognized update repository.' }
    $installedExe = Join-Path $InstallDir $binaryName
    if (-not (Test-Path -LiteralPath $installedExe)) {
        throw 'The installed my-tools.exe was not found.'
    }

    # GitHub Releases API is readable over HTTPS without a GitHub account.
    $headers = @{ 'User-Agent' = 'MyTools-Windows-Updater'; 'Accept' = 'application/vnd.github+json' }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$expectedRepo/releases/latest" -Headers $headers -TimeoutSec 25
    $tag = [string]$release.tag_name
    if ($tag -notmatch '^v(\d+)\.(\d+)\.(\d+)$') { throw "Unexpected release version: $tag" }
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

    $asset = @($release.assets | Where-Object { $_.name -eq $binaryName }) | Select-Object -First 1
    $checksum = @($release.assets | Where-Object { $_.name -eq 'SHA256SUMS.txt' }) | Select-Object -First 1
    if (-not $asset -or -not $checksum) { throw 'Release is missing the EXE or checksum asset.' }
    $assetRoot = "https://github.com/$expectedRepo/releases/download/$tag/"
    if (-not ([string]$asset.browser_download_url).StartsWith($assetRoot, [StringComparison]::Ordinal) -or
        -not ([string]$checksum.browser_download_url).StartsWith($assetRoot, [StringComparison]::Ordinal)) {
        throw 'Release asset URLs did not match the trusted repository.'
    }

    Log "Downloading $tag over HTTPS..."
    if (Test-Path $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    $downloaded = Join-Path $stage $binaryName
    $hashFile = Join-Path $stage 'SHA256SUMS.txt'
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $downloaded -Headers $headers -MaximumRedirection 10 -TimeoutSec 180 -UseBasicParsing
    Invoke-WebRequest -Uri $checksum.browser_download_url -OutFile $hashFile -Headers $headers -MaximumRedirection 10 -TimeoutSec 30 -UseBasicParsing
    $expectedLine = Get-Content $hashFile | Where-Object { $_ -match '^[a-fA-F0-9]{64}\s+my-tools\.exe$' } | Select-Object -First 1
    if (-not $expectedLine) { throw 'Could not find the checksum for my-tools.exe.' }
    $expected = $expectedLine.Substring(0,64).ToUpperInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $downloaded).Hash
    if ($expected -ne $actual) { throw 'SHA-256 mismatch: refusing the downloaded executable.' }

    if ($AgentPid -le 0) { throw 'Missing running process ID.' }
    $tray = [MyToolsNative]::FindWindow('MyToolsAgentWin32', $null)
    if ($tray -eq [IntPtr]::Zero) { throw 'My Tools tray was not found; update deferred.' }
    # WM_APP + 43 gracefully closes the entire My Tools process.
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
