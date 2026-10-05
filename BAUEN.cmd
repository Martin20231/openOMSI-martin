@echo off
rem Baut openOMSI mit dem Spline-Editor. Einfach doppelklicken.
rem Installiert beim ersten Mal (falls noetig) die Visual Studio Build Tools und Rust.
setlocal
cd /d "%~dp0"
set "LOG=%~dp0bau-log.txt"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
rem Zwischendateien ausserhalb von OneDrive (mehrere GB)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\openOMSI-build"
echo ==== openOMSI bauen ==== > "%LOG%"
echo.
echo  openOMSI mit Spline-Editor wird gebaut.
echo  Beim ersten Mal dauert das 30-60 Minuten. Bitte das Fenster offen lassen.
echo  Wenn Windows nach Erlaubnis fragt (Administrator), bitte "Ja" klicken.
echo.

where winget >nul 2>nul
if errorlevel 1 (
  echo FEHLER: winget fehlt. Bitte "App-Installer" aus dem Microsoft Store installieren. >> "%LOG%"
  echo FEHLER: winget fehlt. Bitte "App-Installer" aus dem Microsoft Store installieren.
  goto :ende
)

rem --- Visual Studio Build Tools (C++) ---
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "HASVC="
if exist "%VSWHERE%" (
  for /f "usebackq delims=" %%i in (`"%VSWHERE%" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "HASVC=1"
)
if not defined HASVC (
  echo [1/3] Installiere Visual Studio Build Tools mit C++ ... dauert eine Weile
  echo [1/3] Build Tools werden installiert >> "%LOG%"
  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override "--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" >> "%LOG%" 2>&1
) else (
  echo [1/3] Visual Studio Build Tools sind schon da.
  echo [1/3] Build Tools vorhanden >> "%LOG%"
)

rem --- Rust ---
where cargo >nul 2>nul
if errorlevel 1 (
  echo [2/3] Installiere Rust ...
  echo [2/3] Rust wird installiert >> "%LOG%"
  winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements >> "%LOG%" 2>&1
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo FEHLER: Rust wurde nicht gefunden. Bitte BAUEN.cmd noch einmal starten. >> "%LOG%"
  echo FEHLER: Rust wurde nicht gefunden. Bitte BAUEN.cmd noch einmal starten.
  goto :ende
)
rustup default stable-x86_64-pc-windows-msvc >> "%LOG%" 2>&1
rustc --version >> "%LOG%" 2>&1
if errorlevel 1 (
  echo [2/3] Rust ist unvollstaendig - wird neu installiert ...
  echo [2/3] rustc kaputt, Toolchain neu >> "%LOG%"
  rustup toolchain uninstall stable-x86_64-pc-windows-msvc >> "%LOG%" 2>&1
  rustup toolchain install stable-x86_64-pc-windows-msvc >> "%LOG%" 2>&1
  rustup default stable-x86_64-pc-windows-msvc >> "%LOG%" 2>&1
)
rustc --version >> "%LOG%" 2>&1
if errorlevel 1 (
  echo FEHLER: Rust laesst sich nicht starten. Sag Claude Bescheid. >> "%LOG%"
  echo FEHLER: Rust laesst sich nicht starten. Sag Claude Bescheid.
  goto :ende
)
echo [2/3] Rust ist bereit.

rem --- Bauen ---
echo [3/3] Baue openOMSI (das ist der lange Teil) ...
echo        Das Fenster zeigt dabei nichts Neues an - das ist normal.
echo        Bitte NICHTS druecken (vor allem nicht Strg+C), einfach warten.
echo [3/3] cargo build >> "%LOG%"
cargo build --locked --release -p omsi-app -p omsi-launcher-core >> "%LOG%" 2>&1
if errorlevel 1 goto :fehler
if not exist "%CARGO_TARGET_DIR%\release\openomsi.exe" goto :fehler
goto :kopieren
:fehler
(
  echo.
  echo  FEHLER beim Bauen. Sag Claude Bescheid - die Details stehen in bau-log.txt
  echo BAU FEHLGESCHLAGEN >> "%LOG%"
  goto :ende
)
:kopieren
if not exist "dist\windows" mkdir "dist\windows"
copy /y "%CARGO_TARGET_DIR%\release\openomsi.exe" "dist\windows\openomsi.exe" >nul
copy /y "%CARGO_TARGET_DIR%\release\openomsi-launcher.exe" "dist\windows\openomsi-launcher.exe" >nul
copy /y "assets\steam_redist\steam_api64.dll" "dist\windows\steam_api64.dll" >nul
rem der fertige Spielordner openOMSI-spline (zum Rueberkopieren)
if not exist "openOMSI-spline" mkdir "openOMSI-spline"
copy /y "dist\windows\openomsi.exe" "openOMSI-spline\" >nul
copy /y "dist\windows\openomsi-launcher.exe" "openOMSI-spline\" >nul
for %%f in (dxcompiler.dll dxil.dll msvcp140.dll vcruntime140.dll vcruntime140_1.dll steam_api64.dll) do if exist "%%f" copy /y "%%f" "openOMSI-spline\" >nul
echo BAU ERFOLGREICH >> "%LOG%"
echo.
echo  FERTIG! Das Spiel liegt hier (diesen Ordner kannst du rueberkopieren):
echo  %CD%\openOMSI-spline\openomsi.exe
echo.
echo  Im Spiel: Karte laden, Strg+Umschalt+E (Objekt-Editor), dann X (Splines).
:ende
echo.
pause
