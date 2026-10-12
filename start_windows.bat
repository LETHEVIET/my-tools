@echo off
setlocal
cd /d "%~dp0"
if not exist "desktop\target\release\my-tools.exe" (
  echo Building the Rust tray receiver for the first time...
  call build_windows.bat
  if errorlevel 1 exit /b 1
)
start "" "%~dp0desktop\target\release\my-tools.exe" --minimized %*
