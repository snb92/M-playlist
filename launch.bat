@echo off
echo ==========================================
echo    M-Playlist Live Event Media Server     
echo ==========================================

echo.
echo [1/2] Compiling Native Rust Engine (Release Mode)...
cd m_playlist
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Rust Engine failed to compile!
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo [2/2] Launching C# WPF Brain...
cd ../MPlaylistApp
dotnet run
