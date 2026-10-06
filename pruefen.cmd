@echo off
rem Schnelle Pruefung auf Compiler-Fehler (cargo check), ohne fertige exe.
rem Benutzung: pruefen.cmd    Ergebnis: Konsole und pruef-log.txt
setlocal
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\openOMSI-build"
cargo check --locked --profile fast -p omsi-app -p omsi-launcher-core > pruef-log.txt 2>&1
if errorlevel 1 (
  echo PRUEFUNG FEHLGESCHLAGEN - Details in pruef-log.txt
  findstr /b /c:"error" pruef-log.txt
  exit /b 1
)
echo PRUEFUNG OK
exit /b 0
