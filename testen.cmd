@echo off
rem Tests laufen lassen, z. B.:  testen.cmd -p omsi-launcher-core company::
rem Ergebnis: Konsole (Zusammenfassung) und test-log.txt (alles)
setlocal
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\openOMSI-build"
cargo test --locked --profile fast %* > test-log.txt 2>&1
set "RC=%ERRORLEVEL%"
findstr /c:"test result" /c:"FAILED" /c:"panicked" /c:"error[" /c:"error:" test-log.txt
if "%RC%"=="0" (echo TESTS OK) else (echo TESTS FEHLGESCHLAGEN - Details in test-log.txt)
exit /b %RC%
