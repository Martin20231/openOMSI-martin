# Auftrag: Neues Firmentablet (Schichtbeleg) im Spiel

Lies zuerst `CURSOR-UEBERGABE.md` und die Regeln in `.cursor/rules/`.

## Ziel
Das Firmentablet (F8) ist heute ein normales Listen-Menü mit Textzeilen. Es soll ein eigenes,
frei gezeichnetes Tablet-Fenster werden, das wie die Entwürfe aussieht:

| Bild | Wann es gezeigt wird |
|---|---|
| `docs/firmentablet/1-einstempeln.png` | Tablet offen, noch nicht eingestempelt |
| `docs/firmentablet/2-schichtbeleg.png` | Tablet offen, eingestempelt (Hauptansicht) |
| `docs/firmentablet/3-abrechnung.png` | Direkt nach dem Ausstempeln, bis das Tablet geschlossen wird |

Die Bilder zeigen **Beispieldaten**. Im Spiel kommen echte Werte (siehe „Daten“).
Schrift: die Schrift des Spiels, nicht die aus den Bildern. Farben, Aufteilung, Größen und
Reihenfolge sollen den Bildern so nah wie möglich kommen.

## Was es heute gibt (nicht neu erfinden)
- `crates/omsi-app/src/game_lists.rs`: `ListKind::Tablet`, `tablet_rows()`, `tablet_plan()`,
  Aktion `"clock"` (Ein-/Ausstempeln), Überschrift „Company tablet...“.
- `crates/omsi-app/src/input_script.rs`: `toggle_tablet()` (F8).
- `crates/omsi-app/src/company_lan.rs`: `toggle_company_shift()` (F6), `clock_out_shift()`.
- `crates/omsi-app/src/app.rs`: `shift_on`, `shift_seconds`, `company`, `remote_company`, `duty`.
- `crates/omsi-app/src/app_events.rs` (~Zeile 2076): frischt die Tablet-Zeilen jeden Frame auf.
- `crates/omsi-launcher-core/src/company.rs`: `tuning()` mit `shift_minutes`, `driver_wage`,
  `overtime_extra`; `shift_pay(seconds, planned_seconds, hourly, overtime_extra)` -> `ShiftPay
  { regular_hours, overtime_hours, wage }`; `money()`.
- `crates/omsi-app/src/ui.rs`: das Zeichnen der Menüs. `draw_menu()` und `draw_settings()`
  zeigen, wie man zeichnet: `self.rounded(...)`, `self.shadow(...)`, `self.put(...)`,
  `self.put_right(...)`, `self.chip(...)`, `self.accent_bar(...)`, `menu_scale(f)`.
  Klickbare Flächen kommen in `self.menu_rects` (Index = Zeile in `items`), so funktionieren
  Maus und Tastatur ohne neuen Klick-Code.
- `crates/omsi-app/src/schedule.rs`: `PlayerDuty { line, tour, trips: Vec<PlannedTrip>,
  trip_index, next_stop, .. }`, `PlannedTrip { line, terminus, departure, end, stops }`,
  `schedule::hhmm()`.
- `crates/omsi-app/src/career.rs`: `stops: [gesamt, zu früh, zu spät]`, `metres`, `crashes`.

## Weg (in dieser Reihenfolge, nach jedem Schritt `cmd /c pruefen.cmd`)

### Schritt 1: Daten sammeln (reine Logik, mit Tests)
Neue Datei `crates/omsi-app/src/tablet.rs` (in `lib.rs` mit `mod tablet;` eintragen):
- `pub(crate) struct TabletView` mit allem, was die drei Bilder zeigen:
  Modus (Einstempeln / Schicht / Abrechnung), Uhrzeit, Datum, Firmenkürzel und -farbe,
  Fahrername, Dienstbezeichnung, Dienstzeit, geplante Dauer, Beginn/Ende, Lohn bisher,
  Stundenlohn, Lohn ganze Schicht, Überstunden-Zuschlag, Fahrzeug (Name, Wagennummer,
  Zustand), Pünktlichkeit (pünktlich / gesamt), nächste Abfahrt (Zeit, Linie, Ziel, Minuten bis
  dahin) und die Zeilen des Schichtbelegs.
- `pub(crate) struct DutyRow { time, line, what, detail, arrival, state }` mit
  `state`: Done, Now, Next, Planned, Break (Code-Namen englisch wie im restlichen Projekt).
- `pub(crate) fn tablet_view(app: &App) -> TabletView` füllt das aus den vorhandenen Daten.
- Die Logik für Status und Zeiten (welche Fahrt ist „jetzt“, welche „als Nächstes“, Minuten bis
  zur Abfahrt, Fortschritt der Schicht) als **eigene Funktionen ohne App**, damit man sie testen
  kann. Tests unten in der Datei (Zeiten vor, während und nach dem Dienst; leerer Dienst).

### Schritt 2: Ein eigener Menü-Typ
- In `ui.rs` bei `MenuKind` einen Typ `Tablet` hinzufügen.
- In `game_lists.rs` für `ListKind::Tablet` `MenuKind::Tablet` liefern.
- `Frame` (ui.rs) bekommt `tablet: Option<crate::tablet::TabletView>` (oder eine Referenz),
  gefüllt dort, wo der Frame gebaut wird.
- Die `items` des Tablets bleiben die Aktionen (`clock`, später Reiter, `back`), damit Klicks
  und Tasten wie bisher laufen.

### Schritt 3: Zeichnen
- In `ui.rs` neue Funktion `draw_tablet(...)`, aufgerufen in `draw_menu()` wie
  `draw_settings()`, wenn `f.menu_kind == MenuKind::Tablet`.
- Erst **nur Bild 2 (Schichtbeleg)** bauen, Martin einen Screenshot machen lassen, dann
  Bild 1 und 3.
- Aufbau Bild 2 (Breite ca. 1120 × 740 bei Skalierung 1, mit `menu_scale(f)` skalieren und in
  den Bildschirm einpassen):
  - Rahmen (Tablet-Gehäuse) dunkel `#1a1c20`, Bildschirm `#0e151e`, runde Ecken.
  - Kopfzeile: Firmenkürzel in Firmenfarbe, „Firmentablet“, Dienst · Name; rechts Datum, Uhrzeit.
  - Drei Spalten: links Schicht-Karte (Dienstzeit groß, Fortschrittsbalken, Knopf
    „Ausstempeln“) und „Nächste Abfahrt“; Mitte der Schichtbeleg als Tabelle (Zeit, Linie als
    farbiges Kästchen, Fahrt, Ankunft, Status), die aktuelle Zeile hervorgehoben; rechts
    Fahrzeug, Lohn, Pünktlichkeit (als Balken, kein Ring nötig).
  - Karten: `#16202c`, Rand `#233142`; Text hell `#e8eef5`, grau `#8d9cb1`;
    grün `#4fc28a`, blau `#5aa9ff`, orange `#ff8a4c`; Akzent = Firmenfarbe (Standard `#f2c230`).
  - Reiter unten (Fahrplan, Fahrzeug, Betriebshof, Leitstelle): in diesem Auftrag weglassen
    oder nur „Schichtbeleg“ zeigen. Kommt später.
- Passen nicht alle Fahrten in die Tabelle: die aktuelle und die nächsten zeigen,
  darunter „+ N weitere Fahrten“.
- Kein Dienst (freie Fahrt): Tabelle durch einen kurzen Hinweis ersetzen.
- Die Abrechnung (Bild 3) braucht ein gemerktes Ergebnis der letzten Schicht
  (in `clock_out_shift()` speichern, z. B. Feld `last_shift` in `App`).

### Schritt 4: Texte
Alle sichtbaren Texte englisch im Code, durch `omsi_ui::tr`, deutsch in `app.yml`
(Wörter wie in den Bildern: „Schichtbeleg“, „Einstempeln“, „Nächste Abfahrt“, „als Nächstes“ …).

## Daten: was es gibt und was nicht
- Dienstbezeichnung: `duty.line` + `duty.tour` (z. B. „136-04“). Kein Dienst: „Freie Fahrt“.
- Geplante Dauer: `tuning().shift_minutes` (Standard 20 min, Martin kann sie ändern).
- Lohn: nur `shift_pay(...)`. **Keine neuen Prämien oder Regeln erfinden.** Die
  „Pünktlichkeitsprämie“ im Bild 3 weglassen, solange es sie in `company.rs` nicht gibt.
- Pünktlichkeit: aus `career.stops` (gesamt minus zu spät).
- Fahrzeug: Name aus `player.vehicle.ty.def`; Wagennummer und Zustand aus der Firma
  (`company.buses`, Abgleich über `driven_bus_file()` und `company::same_file`).
- Tank: nur zeigen, wenn es einen einfachen vorhandenen Wert gibt; sonst die Zeile weglassen.
- Stellplatz-Plan (Bild 1, rechts): erst mal weglassen oder nur „Dein Bus: Wagen …“.
- Im Mehrspieler als Freund (`remote_company`): Firmenkürzel vom Host, Lohn wie heute.

## Fertig, wenn
- `cmd /c pruefen.cmd` ohne Fehler, Tests in `tablet.rs` grün, `cmd /c bau-schritt.cmd fast` erfolgreich.
- F8 öffnet das neue Tablet, F8 oder „Zurück“ schließt es, Ein-/Ausstempeln per Klick und F6 geht.
- Uhrzeit, Dienstzeit und Lohn laufen sichtbar weiter, während es offen ist.
- Martin hat Screenshots geschickt und ist zufrieden.
