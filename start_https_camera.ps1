# Optional development-only HTTPS link for the in-app iPhone camera.
# Photos transit Cloudflare. Leave this console open during the session.
param([int]$Port = 8765)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$exe = Join-Path $PSScriptRoot "desktop\target\release\my-tools.exe"
if (-not (Test-Path $exe)) {
    throw "Build the receiver first with build_windows.bat."
}
$cloudflared = Get-Command cloudflared -ErrorAction SilentlyContinue
if (-not $cloudflared) {
    throw "Install cloudflared first (winget install Cloudflare.cloudflared). Then reopen PowerShell."
}
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Port $Port is already in use. Exit My Tools from its tray menu before starting HTTPS mode."
}
$stderr = Join-Path $env:TEMP "my-tools-cloudflared-$PID.log"
$stdout = Join-Path $env:TEMP "my-tools-cloudflared-$PID.out"
$tunnel = $null
$receiver = $null
try {
    Write-Host "Starting temporary HTTPS camera link..."
    $tunnel = Start-Process -FilePath $cloudflared.Source -ArgumentList @("tunnel", "--no-autoupdate", "--url", "http://127.0.0.1:$Port") -WindowStyle Hidden -RedirectStandardError $stderr -RedirectStandardOutput $stdout -PassThru
    $url = $null
    for ($i = 0; $i -lt 80; $i++) {
        if ($tunnel.HasExited) { throw "cloudflared exited unexpectedly; check $stderr" }
        Start-Sleep -Milliseconds 500
        $logs = Get-Content $stderr -Raw -ErrorAction SilentlyContinue
        if ($logs -match 'https://[a-zA-Z0-9-]+\.trycloudflare\.com') {
            $url = $Matches[0]
            break
        }
    }
    if (-not $url) { throw "Could not find a Cloudflare quick-tunnel URL. Check $stderr" }
    Write-Host "Secure camera URL: $url"
    Write-Host "A QR code will open in your PC browser. Scan it with your iPhone."
    Write-Host "Photos travel through Cloudflare in this optional mode, not directly over local Wi-Fi."
    $receiver = Start-Process -FilePath $exe -ArgumentList @("--minimized", "--port", "$Port", "--public-url", $url, "--show") -PassThru
    Wait-Process -Id $receiver.Id
} finally {
    if ($tunnel -and -not $tunnel.HasExited) { Stop-Process -Id $tunnel.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item $stderr, $stdout -ErrorAction SilentlyContinue
}
