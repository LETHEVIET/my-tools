@echo off
setlocal
cd /d "%~dp0"
rem GPUI Kit requires CMake and the MSVC/Windows SDK toolchain.
if exist ".venv\Scripts\cmake.exe" set "PATH=%~dp0.venv\Scripts;%PATH%"
where cmake >nul 2>nul
if errorlevel 1 (
  echo CMake is missing. Install it via winget install Kitware.CMake
  echo Or install into the project .venv with: .venv\Scripts\python.exe -m pip install cmake
  exit /b 1
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust is missing. Install the MSVC toolchain from https://rustup.rs/
  exit /b 1
)
cargo build --release --locked --manifest-path desktop\Cargo.toml
if errorlevel 1 exit /b 1
echo Built: desktop\target\release\my-tools.exe
