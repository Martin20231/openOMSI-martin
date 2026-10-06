@echo off
rem Zeigt das Bau-Log live an. Schliessen mit dem X oder Strg+C (das stoppt NUR die Anzeige, nicht den Bau).
cd /d "%~dp0"
title openOMSI Bau-Log (live)
powershell -NoProfile -Command "Get-Content -Path .\bau-log.txt -Tail 30 -Wait"
