@echo off
setlocal
cd /d "%~dp0"
if not exist "rust\target\release\snap-to-prompt.exe" (
  echo Building the Rust tray receiver for the first time...
  call build_windows.bat
  if errorlevel 1 exit /b 1
)
start "" "%~dp0rust\target\release\snap-to-prompt.exe" %*
