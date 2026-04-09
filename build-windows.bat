@echo off
REM Vocal Typewriter AI - Windows Build Script
REM Builds both x64 and ARM64 Windows versions with MSI and NSIS installers

echo ========================================================
echo Vocal Typewriter AI - Windows Build Script
echo ========================================================
echo.

REM Check Rust
where cargo >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Rust not found. Install from https://rustup.rs
    pause
    exit /b 1
)
echo [OK] Rust found

REM Check bun
where bun >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [INFO] Installing bun...
    powershell -c "irm bun.sh/install.ps1 | iex"
    if %ERRORLEVEL% NEQ 0 (
        echo [ERROR] Failed to install bun
        pause
        exit /b 1
    )
)
echo [OK] Bun found

REM Add Windows targets
echo.
echo [INFO] Adding Windows build targets...
rustup target add x86_64-pc-windows-msvc 2>nul || echo Target x86_64 already installed
rustup target add aarch64-pc-windows-msvc 2>nul || echo Target aarch64 already installed

REM Install dependencies
echo.
echo [INFO] Installing dependencies...
call bun install
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Failed to install dependencies
    pause
    exit /b 1
)

REM Create output directory
if not exist "release-windows" mkdir "release-windows"

REM Build x64 version
echo.
echo ========================================================
echo Building Windows x64 version...
echo ========================================================
set CARGO_BUILD_JOBS=2
call bunx tauri build --target x86_64-pc-windows-msvc --bundles nsis,msi
if %ERRORLEVEL% NEQ 0 (
    echo [WARN] x64 bundling failed. Attempting portable exe fallback...
    if exist "src-tauri\target\x86_64-pc-windows-msvc\release\vocal-typewriter.exe" (
        copy /Y "src-tauri\target\x86_64-pc-windows-msvc\release\vocal-typewriter.exe" "release-windows\Vocal-Typewriter-AI-x64-portable.exe" >nul 2>&1
        echo [OK] x64 portable exe prepared
    ) else (
        echo [ERROR] x64 build failed and no portable exe found
        pause
        exit /b 1
    )
)

REM Copy x64 artifacts
echo.
echo [INFO] Copying x64 artifacts...
copy /Y "src-tauri\target\x86_64-pc-windows-msvc\release\bundle\msi\*.msi" "release-windows\" >nul 2>&1
copy /Y "src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\*.exe" "release-windows\" >nul 2>&1

REM Build ARM64 version
echo.
echo ========================================================
echo Building Windows ARM64 version...
echo ========================================================
set CARGO_BUILD_JOBS=2
call bunx tauri build --target aarch64-pc-windows-msvc --bundles nsis,msi
if %ERRORLEVEL% NEQ 0 (
    echo [WARN] ARM64 bundling failed. Attempting portable exe fallback...
    if exist "src-tauri\target\aarch64-pc-windows-msvc\release\vocal-typewriter.exe" (
        copy /Y "src-tauri\target\aarch64-pc-windows-msvc\release\vocal-typewriter.exe" "release-windows\Vocal-Typewriter-AI-arm64-portable.exe" >nul 2>&1
        echo [OK] ARM64 portable exe prepared
    ) else (
        echo [ERROR] ARM64 build failed and no portable exe found
        pause
        exit /b 1
    )
)

REM Copy ARM64 artifacts
echo.
echo [INFO] Copying ARM64 artifacts...
copy /Y "src-tauri\target\aarch64-pc-windows-msvc\release\bundle\msi\*.msi" "release-windows\" >nul 2>&1
copy /Y "src-tauri\target\aarch64-pc-windows-msvc\release\bundle\nsis\*.exe" "release-windows\" >nul 2>&1

REM Summary
echo.
echo ========================================================
echo Build Complete!
echo ========================================================
echo.
echo Output directory: release-windows\
echo.
dir /b "release-windows\" 2>nul || echo No files found
echo.
echo Build artifacts:
echo   - MSI installers (Windows Installer)
echo   - EXE installers (NSIS)
echo   - Both x64 and ARM64 architectures
echo.
pause
