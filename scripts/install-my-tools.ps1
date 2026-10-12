# Run from an extracted official My Tools release archive.
param([switch]$SkipLaunch)
$ErrorActionPreference = 'Stop'
$source = $PSScriptRoot
$destination = Join-Path $env:LOCALAPPDATA 'Programs\MyTools'
foreach ($name in @('my-tools.exe','my-tools-agent.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $name))) {
        throw "Missing $name. Extract the whole release ZIP before installation."
    }
}
if (Get-Process -Name 'my-tools-agent','my-tools','snap-to-prompt' -ErrorAction SilentlyContinue) {
    throw 'Exit the running My Tools / Snap to Prompt tray and desktop apps, then run the installer again.'
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach ($name in @('my-tools.exe','my-tools-agent.exe')) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination (Join-Path $destination $name) -Force
}
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
New-Item -Path $run -Force | Out-Null
Set-ItemProperty -Path $run -Name 'MyTools' -Value ('"' + (Join-Path $destination 'my-tools-agent.exe') + '"')
Remove-ItemProperty -Path $run -Name 'SnapToPrompt' -ErrorAction SilentlyContinue
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $startMenu 'My Tools.lnk'))
$shortcut.TargetPath = Join-Path $destination 'my-tools.exe'
$shortcut.WorkingDirectory = $destination
$shortcut.Description = 'My Tools desktop hub'
$shortcut.Save()
Write-Host "My Tools installed to $destination"
Write-Host "Starts the lightweight background agent at Windows login."
Write-Host "For private GitHub updates, install gh and authenticate with gh auth login."
if (-not $SkipLaunch) {
    Start-Process -FilePath (Join-Path $destination 'my-tools-agent.exe')
    Start-Process -FilePath (Join-Path $destination 'my-tools.exe')
}
