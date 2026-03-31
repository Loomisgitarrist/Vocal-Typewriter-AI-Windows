@echo off
setlocal enableextensions
cd /d "%~dp0"
title Install Vocal Typewriterer

:: Initialize variables
set "CERT_FILE=ca.crt"
set "APP_EXE=Vocal Typewriterer_1.0.34_x64-setup.exe"

echo ===================================================
echo      Vocal Typewriterer - Installation Helper
echo ===================================================
echo.
echo Working Directory: %~dp0
echo.

:: ---------------------------------------------------------
:: 1. SMART FILE DETECTION
:: ---------------------------------------------------------

:: A) Check for the Installer
if not exist "%APP_EXE%" (
    color 4F
    echo [ERROR] INSTALLER MISSING: "%APP_EXE%"
    echo.
    echo Please make sure the .exe file is in this folder.
    echo.
    echo Files found in this folder:
    dir /b *.exe
    echo.
    pause
    exit /b 1
)

:: B) Check for the Certificate (Smart Search)
if exist "%CERT_FILE%" goto CertFound

:: Fallback 1: Check for double extension (Common mistake: ca.crt.crt)
if exist "ca.crt.crt" (
    set "CERT_FILE=ca.crt.crt"
    goto CertFound
)

:: Fallback 2: Check for ANY .crt file
if exist "*.crt" (
    for %%f in (*.crt) do set "CERT_FILE=%%f"
    goto CertFound
)

:: If we get here, no certificate was found
color 4F
echo [ERROR] CERTIFICATE MISSING!
echo.
echo I could not find "ca.crt" in this folder.
echo.
echo Troubleshooting:
echo 1. Did you extract the ZIP file?
echo 2. Is the file named "ca.crt" or just "ca"?
echo.
echo Files found in this folder:
dir /b
echo.
pause
exit /b 1

:CertFound
echo [OK] Found Certificate: "%CERT_FILE%"
echo [OK] Found Installer:   "%APP_EXE%"
echo.

:: ---------------------------------------------------------
:: 2. CHECK FOR ADMIN RIGHTS
:: ---------------------------------------------------------
>nul 2>&1 "%SYSTEMROOT%\system32\cacls.exe" "%SYSTEMROOT%\system32\config\system"
if '%errorlevel%' NEQ '0' (
    echo [INFO] Requesting Admin Privileges...
    goto UACPrompt
) else ( goto gotAdmin )

:UACPrompt
    echo Set UAC = CreateObject^("Shell.Application"^) > "%temp%\getadmin.vbs"
    echo UAC.ShellExecute "%~s0", "", "", "runas", 1 >> "%temp%\getadmin.vbs"
    "%temp%\getadmin.vbs"
    exit /B

:gotAdmin
    if exist "%temp%\getadmin.vbs" ( del "%temp%\getadmin.vbs" )
    cd /d "%~dp0"

    :: ---------------------------------------------------------
    :: 3. INSTALLATION
    :: ---------------------------------------------------------
    
    echo.
    echo [1/2] Installing Certificate...
    echo       Target: "%CERT_FILE%"
    
    certutil -addstore -f "Root" "%CERT_FILE%"
    
    if %errorlevel% NEQ 0 (
        color 4F
        echo.
        echo [ERROR] Failed to install certificate.
        echo Error Code: %errorlevel%
        pause
        exit /b %errorlevel%
    )
    
    echo.
    echo [SUCCESS] Certificate Installed.
    echo.
    
    echo [2/2] Launching Installer...
    echo       Running: "%APP_EXE%"
    echo.
    
    "%APP_EXE%"
    
    if %errorlevel% NEQ 0 (
        echo.
        echo [WARNING] Installer exited with code %errorlevel%.
    )

    echo.
    echo Done.
    pause
