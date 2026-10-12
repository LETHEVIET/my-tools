@echo off
setlocal
cd /d "%~dp0"
if not exist "desktop\target\release\my-tools.exe" (
  call build_hub_windows.bat
  if errorlevel 1 exit /b 1
)
start "" "%~dp0desktop\target\release\my-tools.exe"
