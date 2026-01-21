@echo off
REM Build script for Linux using cross
REM Requires Docker and cross to be installed

echo ========================================
echo Building spire for Linux using cross
echo ========================================

cd /d "%~dp0rust-proxy"

REM Check if cross is installed
where cross >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] cross is not installed or not in PATH
    echo Please install cross with: cargo install cross
    exit /b 1
)

REM Check if Docker is running
docker ps >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Docker is not running or not installed
    echo Please start Docker Desktop
    exit /b 1
)

echo.
echo [1/1] Building for x86_64-unknown-linux-gnu...
cross build --release --target x86_64-unknown-linux-gnu
if %errorlevel% neq 0 (
    echo [ERROR] Build failed for x86_64-unknown-linux-gnu
    exit /b 1
)


echo.
echo ========================================
echo Build completed successfully!
echo ========================================
echo.
echo Output binaries:
echo - target\x86_64-unknown-linux-gnu\release\spire
echo.

cd /d "%~dp0"
