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
call bun tauri build --target x86_64-pc-windows-msvc
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] x64 build failed
    pause
    exit /b 1
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
call bun tauri build --target aarch64-pc-windows-msvc
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] ARM64 build failed
    pause
    exit /b 1
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
