@echo off
REM Test script for distributed downloader on Windows

REM Create directories
mkdir ddr-download\test-manager 2>nul
mkdir ddr-download\test-server 2>nul
mkdir ddr-download\test-client\tmp 2>nul
mkdir ddr-download\test-client\target 2>nul

REM Start manager in background
echo Starting manager...
start "Manager" cmd /c "cargo run --bin manager -- --config configs/manager.yml"

REM Wait a bit for manager to start
timeout /t 2 /nobreak >nul

REM Start server in background
echo Starting server...
start "Server" cmd /c "cargo run --bin server -- --config configs/server.yml"

REM Wait a bit for server to start
timeout /t 2 /nobreak >nul

REM Try to download a small file
echo Attempting to download test file...
cargo run --bin client -- https://httpbin.org/json --config configs/test_client.yml

echo Test completed.
pause