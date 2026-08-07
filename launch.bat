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
echo [1.5/2] Deploying Engine DLL...
copy /Y target\release\m_playlist.dll ..\MPlaylistApp\bin\Debug\net8.0-windows\m_playlist.dll >nul
copy /Y target\release\m_playlist.dll ..\MPlaylistApp\bin\Release\net8.0-windows\m_playlist.dll >nul

echo.
echo [2/2] Launching C# WPF Brain...
cd ../MPlaylistApp
dotnet run
