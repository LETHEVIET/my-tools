@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust is required to build the receiver. Install it from https://rustup.rs/
  exit /b 1
)
cargo build --release --locked --manifest-path rust\Cargo.toml
if errorlevel 1 exit /b 1
echo Built: rust\target\release\snap-to-prompt.exe
