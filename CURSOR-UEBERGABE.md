# Übergabe: openOMSI-Erweiterungen von Martin (für Cursor)

Diese Datei beschreibt, was schon gebaut ist, wie gebaut und getestet wird und was noch offen ist.
Lies sie ganz, bevor du etwas änderst.

## Über Martin
- Programmierer, kann aber (noch) kein Rust. Erkläre auf Deutsch, einfach und kurz.
- Er spielt auf einem zweiten PC und kopiert den Ordner `openOMSI-spline` per RDP dorthin.

## Projekt
- openOMSI: Open-Source-Nachbau von OMSI 2 in Rust (Workspace unter `crates/`, wgpu + winit).
- Wichtige Crates: `omsi-app` (Spiel + Launcher in einer exe), `omsi-launcher-core`
  (Lib-Name `omsi_launcher_lib`, Logik ohne Grafik), `omsi-map`, `omsi-ui`, `omsi-net`, `omsi-sim`.
- Texte: Englisch ist der Schlüssel, Übersetzung über `omsi_ui::tr("English text")`.
  Deutsche Texte stehen in `crates/omsi-app/locales/app.yml`:
  ```yaml
  "English text":
    de: "Deutscher Text"
  ```
  Jeder neue sichtbare Text braucht einen deutschen Eintrag. Doppelte Schlüssel vermeiden.
- Code-Stil des Projekts: Kommentare in Englisch, ganze Sätze, `(…)` für Nebenbemerkungen.
  Bestehende Muster nachmachen, keine neuen Abhängigkeiten (Crates) hinzufügen.

## Bauen und testen (Windows)
- Für Cursor am einfachsten (Details in `.cursor/rules/openomsi-bauen.mdc`):
  `cmd /c pruefen.cmd` (Fehler finden, ~1 Min), `cmd /c testen.cmd -p <crate> <modul>::`,
  `cmd /c bau-schritt.cmd fast` (fertige exe nach `openOMSI-spline`).
- `AUTO-BAUEN.cmd` läuft im Hintergrund und wartet auf die Datei `bau-auftrag.txt`.
  - Inhalt `fast` = schneller Bau (Profil `fast` in der Cargo.toml), `release` = voller Bau.
  - Ergebnis: `bau-status.txt` (LAEUFT / ERFOLGREICH / FEHLGESCHLAGEN / NICHT-KOPIERT)
    und `bau-log.txt`. Bei Erfolg wird `openOMSI-spline\` (exe + DLLs) aktualisiert.
  - Steht `bau-tests.txt` im Ordner, läuft nach dem Bau jede Zeile als
    `cargo test --locked --profile fast <zeile>`, z. B. `-p omsi-launcher-core company::`.
    Ausgabe in `test-log.txt`, am Ende steht `TESTS FERTIG`.
- `AUTO-BAUEN.cmd` nicht ändern, solange es läuft.
- Von Hand geht auch: `cargo build --locked --profile fast -p omsi-app -p omsi-launcher-core`
  (CARGO_TARGET_DIR ist `%LOCALAPPDATA%\openOMSI-build`, siehe `bau-schritt.cmd`).
- Ein Bau dauert ca. 3–6 Minuten. Nach jeder Änderung bauen und `bau-log.txt` auf `error` prüfen.

## Was schon fertig ist (alles gebaut und getestet)

### 1. Spline-Editor (Straßen bearbeiten im Objekt-Editor)
- `crates/omsi-map/src/tile_write.rs` – schreibt geänderte Splines in die .map-Datei (6 Tests).
- `crates/omsi-app/src/spline_editor.rs` – Bearbeiten (Op: Move, Turn, Length, Curve, Straight,
  Grade, Delete, Undo, Continue, Attach, PullChain, SetFile), 11 Tests.
- `crates/omsi-app/src/editor.rs`, `input_script.rs` (`spline_editor_key`, `editor_save`,
  `editor_reload_frame`), `scene.rs` (`spline_edits`), `app_events.rs` (Markierungen, Hilfetext).
- Im Spiel: Objekt-Editor öffnen, Taste X = Spline-Modus. Doku: `docs/USER_GUIDE.md`.

### 2. Busunternehmen (Launcher-Seite „Unternehmen“)
- Logik: `crates/omsi-launcher-core/src/company.rs` (nur std, 14 Tests). Company, Bus, Hire,
  Level, Run, RunPrice, `price()`, `book()`, Kaufen/Verkaufen/Werkstatt, Hinweise, `money()`.
- Werte zum Anpassen ohne neu bauen: `company-values.cfg` (deutsche Schlüssel, im
  `.openomsi`-Ordner). Launcher-Button „Werte bearbeiten“ öffnet sie im Editor.
- Launcher-Seite: `crates/omsi-app/src/launcher/company.rs`
  (Gründen, Übersicht, Fuhrpark, Linien, Personal, Finanzen).
- Fahrten werden aus den Sitzungsdateien `~/.openomsi/sessions/*.json` gebucht
  (`company_runs()` in `omsi-launcher-core/src/lib.rs`).

### 3. Unternehmen im Spiel
- Oben links: „BVG · Kontostand … · diese Fahrt …“ (`App::company_line()` in `app.rs`,
  berechnet pro Frame in `app_events.rs` als `company_hud`).
- Firmenbusse fahren als KI auf den Firmenlinien (`schedule.rs`: `set_company_fleet`,
  `company_vehicle`). Pro angestelltem Fahrer einer Linie ein Bus.
- Mehrspieler: `crates/omsi-app/src/company_lan.rs` (Host sendet Kontostand, Freunde senden
  ihre Fahrten, Host schreibt sie als Sitzungsdatei). Befehle `company`/`comprun` in `admin.rs`.
- Sprache: Das Spiel übernimmt die Sprache vom Launcher über die Umgebungsvariable
  `OMSI_LANGUAGE` (`ui_language()` in `lib.rs`, `Settings::load()` in `settings.rs`).

### 4. Betriebshof in 3D (neu, gebaut, noch NICHT im Spiel ausprobiert)
- `crates/omsi-app/src/depot.rs`: `park_company_buses()` stellt die Firmenbusse, die gerade
  keine Linie fahren, als „placed vehicles“ (`App::placed`) auf die Startpunkte (Entry Points)
  des Betriebshofs. Reichen die Plätze nicht, stehen die übrigen in Reihen daneben
  (`rows_beside()`, 4,6 m Abstand, Reihen 16 m tief, 6 pro Reihe).
- Aufruf beim Weltstart in `app.rs` (direkt nach dem Laden der Firma).
- Einsteigen: zu Fuß (Strg+Umschalt+G) zur Fahrertür laufen und G drücken. Das gab es schon
  (`on_foot.rs`: `placed_cab_near`, `take_placed`).
- `company.rs`: `depot_base()`, `depot_names()`, `depot_entries()`, `depot_buses()`
  (abgenutzte Busse unter 30 % stehen am Ende, höchstens `betriebshof_max_busse`, Standard 12).
- `App::driven_bus_file()` in `depot.rs`: Die Fahrt zählt für den Bus, der zuletzt gefahren
  wurde (nicht den Startbus). Wird in `input_script.rs` (Sitzung schreiben),
  `app.rs` (`company_line`) und `company_lan.rs` benutzt.
- Launcher (Unternehmen → Übersicht, rechts unten): Auswahl „Betriebshof“ (aus den
  Startpunkten der Karte) und Button „Am Betriebshof starten“ (freie Fahrt, bester Firmenbus,
  Startpunkt = Betriebshof).

## Offen / nächste Schritte
1. Betriebshof im Spiel testen (Martin schickt Screenshots). Wahrscheinliche Anpassungen:
   - Reihen stehen auf Gras oder in Wänden: Abstände in `depot.rs` anpassen oder die Reihen
     auf die linke Seite / nach hinten legen.
   - Ladezeit mit vielen Bussen: `betriebshof_max_busse` in `company-values.cfg` senken.
   - Prüfen, ob Busse ineinander stehen (dann den Mindestabstand 4 m erhöhen).
2. Preise und Verschleiß ausbalancieren (nur `company-values.cfg`, kein Bau nötig).
3. Später vielleicht ein Pull Request an das Original-Projekt (openOMSI-Project/openOMSI).

## Regeln für die Arbeit
- Erst den betroffenen Code lesen, dann ändern. Kleine Schritte, nach jedem Schritt bauen.
- Tests für reine Logik schreiben (wie in `company.rs`, `depot.rs`) und über `bau-tests.txt` laufen lassen.
- Nie behaupten, etwas funktioniere im Spiel, bevor Martin es ausprobiert hat.
- Keine Dateien löschen, ohne Martin zu fragen.

### 5. Disponent im Mehrspieler (Etappe 1, gebaut, noch nicht im Spiel getestet)
- `crates/omsi-app/src/dispatch.rs`: Rollen (Host = Chef, ein Disponent), Dienstplan aus den Kursen
  des Fahrplans, Zuteilen, Anfrage an den Fahrer, Annehmen/Ablehnen. Reine Logik mit Tests
  (`encode_plan`, `apply_plan`, `give`, `free`, `answer`, `encode_ask`, `parse_ask`).
- LAN-Befehle `dispo role|lines|plan|ask|give|free|answer` (Felder getrennt mit `¦`), Weiterleitung
  in `input_script.rs` (`lan_command`). Der Host schickt den Plan alle 5 s (`dispatch_tick`).
- Tablet: Reiter „Leitstelle“ (Tab 4) zeigt für Chef und Disponent das Pult (`ui.rs`: `tablet_board`),
  die Anfrage liegt als Karte über jedem Reiter (`tablet_request`). Knöpfe: `dline`, `drow`, `dgive`,
  `dfree`, `dup`, `ddown`, `drole`, `daccept`, `ddecline` (`dispatch::tablet_items` / `tablet_action`).
- Angenommen startet der Dienst wie aus dem Menü (`game_lists::start_duty`).
- Noch offen: Etappe 2 (Live-Status/Verspätung im Pult, Bus-Auswahl), Etappe 3 (Funk an alle).
