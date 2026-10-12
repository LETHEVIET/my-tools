@echo off
setlocal
cd /d "%~dp0"
echo Before upgrading, right-click the Snap to Prompt tray icon and choose Exit.
echo Windows will not overwrite an EXE while it is running.
pause
call build_windows.bat
if errorlevel 1 (echo Build failed. & pause & exit /b 1)
call start_windows.bat --show
