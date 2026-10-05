@echo off
rem Ein Bau von openOMSI (von AUTO-BAUEN.cmd aufgerufen): "fast" zum Testen, "release" fuer die
rem fertige Version. Schreibt bau-log.txt und bau-status.txt und aktualisiert openOMSI-spline.
setlocal
cd /d "%~dp0"
set "LOG=%~dp0bau-log.txt"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\openOMSI-build"
set "PROFIL=%~1"
if not "%PROFIL%"=="release" set "PROFIL=fast"
echo LAEUFT %PROFIL% %DATE% %TIME%> bau-status.txt
echo ==== openOMSI bauen (%PROFIL%) %DATE% %TIME% ==== > "%LOG%"
cargo build --locked --profile %PROFIL% -p omsi-app -p omsi-launcher-core >> "%LOG%" 2>&1
if errorlevel 1 goto fehler
if not exist "%CARGO_TARGET_DIR%\%PROFIL%\openomsi.exe" goto fehler
if not exist "openOMSI-spline" mkdir "openOMSI-spline"
set "KOPIERT=ja"
copy /y "%CARGO_TARGET_DIR%\%PROFIL%\openomsi.exe" "openOMSI-spline\" >nul || set "KOPIERT=nein"
copy /y "%CARGO_TARGET_DIR%\%PROFIL%\openomsi-launcher.exe" "openOMSI-spline\" >nul || set "KOPIERT=nein"
for %%f in (dxcompiler.dll dxil.dll msvcp140.dll vcruntime140.dll vcruntime140_1.dll steam_api64.dll) do if exist "%%f" copy /y "%%f" "openOMSI-spline\" >nul
if "%KOPIERT%"=="nein" (
  echo BAU ERFOLGREICH, ABER NICHT KOPIERT - lief openOMSI noch? >> "%LOG%"
  echo NICHT-KOPIERT %PROFIL% %DATE% %TIME%> bau-status.txt
  exit /b 2
)
echo BAU ERFOLGREICH >> "%LOG%"
rem Tests, wenn Claude welche bestellt hat (bau-tests.txt: eine Zeile pro cargo-test-Aufruf)
if exist "bau-tests.txt" (
  echo ==== Tests %DATE% %TIME% ==== > "test-log.txt"
  for /f "usebackq delims=" %%t in ("bau-tests.txt") do cargo test --locked --profile %PROFIL% %%t >> "test-log.txt" 2>&1
  del "bau-tests.txt" >nul 2>nul
  echo TESTS FERTIG >> "test-log.txt"
)
echo ERFOLGREICH %PROFIL% %DATE% %TIME%> bau-status.txt
exit /b 0
:fehler
echo BAU FEHLGESCHLAGEN >> "%LOG%"
echo FEHLGESCHLAGEN %PROFIL% %DATE% %TIME%> bau-status.txt
exit /b 1
