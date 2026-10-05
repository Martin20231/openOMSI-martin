@echo off
rem Wartet auf Bau-Auftraege von Claude (die Datei bau-auftrag.txt) und baut dann openOMSI.
rem Einfach starten und das Fenster offen lassen. Schliessen beendet nur das Warten.
title openOMSI Auto-Bau - bitte offen lassen
cd /d "%~dp0"
echo.
echo  openOMSI Auto-Bau laeuft. Dieses Fenster einfach offen lassen.
echo  Claude startet die Baus selbst. Das fertige Spiel liegt immer in openOMSI-spline.
echo  Vor einem Bau bitte openOMSI schliessen, sonst kann es nicht ersetzt werden.
echo.
echo WARTET %DATE% %TIME%> bau-status.txt
:warte
if exist "bau-auftrag.txt" goto bauen
timeout /t 3 /nobreak >nul
goto warte
:bauen
set "PROFIL="
for /f "usebackq tokens=1" %%a in ("bau-auftrag.txt") do set "PROFIL=%%a"
del "bau-auftrag.txt" >nul 2>nul
if not "%PROFIL%"=="release" set "PROFIL=fast"
echo  [%TIME%] Baue (%PROFIL%) ...
call "%~dp0bau-schritt.cmd" %PROFIL%
if errorlevel 2 (
  echo  [%TIME%] Gebaut, aber openOMSI lief noch - bitte schliessen, Claude baut nochmal.
) else if errorlevel 1 (
  echo  [%TIME%] Fehler beim Bauen - Claude kuemmert sich darum.
) else (
  echo  [%TIME%] Fertig. openOMSI-spline ist aktuell.
)
goto warte
