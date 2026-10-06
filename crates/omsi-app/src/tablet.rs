//! The company tablet: the shift ticket, gathered from the session.
//!
//! What is on the screen (the three pictures) is [`TabletView`]. Which trip is "now",
//! which is next, and how far the shift has run are plain functions, so they can be
//! tested without the game.

use crate::App;

/// Which of the three tablet screens is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabletMode {
    /// Not clocked in yet (picture 1).
    ClockIn,
    /// Clocked in: the shift ticket (picture 2).
    OnDuty,
    /// Clocking out, until the tablet is closed or the settlement is confirmed (picture 3).
    Settlement,
}

/// Career counters at clock-in, so the settlement can show what this shift changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShiftSnap {
    pub metres: f64,
    pub stops: [i32; 3],
    pub crashes: i32,
}

/// What the settlement shows for one shift. The wage itself comes from `shift_pay`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShiftBill {
    pub seconds: f64,
    pub km: f64,
    pub on_time: i32,
    pub stops: i32,
    pub crashes: i32,
}

/// The duty card on the clock-in screen, from the planned trips.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DutyFacts {
    pub start: String,
    pub end: String,
    pub line: String,
    pub route: String,
    pub trips: usize,
    pub breaks: usize,
    pub break_min: i64,
    pub first: String,
}

/// The pay slip. No bonus that `shift_pay` does not already pay.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slip {
    pub time: String,
    pub overtime: String,
    pub km: String,
    pub trips: usize,
    pub punctual: Option<(i32, i32)>,
    pub regular: String,
    pub regular_pay: String,
    pub overtime_line: String,
    pub overtime_pay: String,
    pub crashes: i32,
    pub wage: String,
}

/// Where one line of the shift ticket stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowState {
    Done,
    Now,
    Next,
    Planned,
    Break,
}

/// One line of the shift ticket.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DutyRow {
    pub time: String,
    pub line: String,
    pub what: String,
    pub detail: String,
    pub arrival: String,
    pub state: RowState,
}

/// The next departure the left card shows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NextLeave {
    pub time: String,
    pub line: String,
    pub dest: String,
    pub from: String,
    pub minutes: i64,
}

/// Everything the tablet draws. Example numbers in the pictures are not used:
/// these are the session's own.
#[derive(Clone, Debug)]
pub(crate) struct TabletView {
    pub mode: TabletMode,
    /// 0 shift ticket, 1 timetable, 2 vehicle, 3 depot, 4 control room.
    pub tab: u8,
    /// "Good morning" and the like, already translated.
    pub hello: String,
    pub facts: Option<DutyFacts>,
    /// "Run 4", already translated, or empty.
    pub tour_label: String,
    pub depot: String,
    /// Which of six bays the picture marks. There is no real stall on the map:
    /// this is only a stable place from the fleet number.
    pub stall: Option<u8>,
    pub slip: Option<Slip>,
    pub clock: String,
    pub date: String,
    pub short: String,
    pub color: [u8; 4],
    pub driver: String,
    pub duty_name: String,
    pub free_run: bool,
    pub duty_clock: String,
    pub planned_label: String,
    pub progress: f32,
    pub overtime: bool,
    pub shift_start: String,
    pub shift_end: String,
    pub wage_now: String,
    pub hourly: String,
    pub wage_full: String,
    pub overtime_pct: String,
    pub bus_name: String,
    pub bus_nr: String,
    pub condition: Option<f32>,
    /// Tank, 0 to 1, when the bus's script has one.
    pub tank: Option<f32>,
    /// Odometer in kilometres, when the bus has one.
    pub km: Option<f64>,
    /// Stops on time, and stops served.
    pub punctual: Option<(i32, i32)>,
    pub next: Option<NextLeave>,
    pub rows: Vec<DutyRow>,
    /// Stops of the trip under way (the timetable page).
    pub stops: Vec<StopLine>,
    pub stop_title: String,
    /// The company line this bus is on. Empty: it waits in the depot, or it is not a company bus.
    pub bus_line: String,
    pub yard: Vec<YardBus>,
    /// How many of `yard` are not out on a line.
    pub yard_home: usize,
    /// A friend in multiplayer: the fleet lives on the host's computer.
    pub fleet_remote: bool,
    pub next_stop: String,
    pub next_stop_time: String,
    pub notes: Vec<DeskNote>,
    /// The control room of the boss and the dispatcher in multiplayer (the last tab).
    pub board: Option<crate::dispatch::BoardView>,
    /// A duty the control room asks this player to drive.
    pub request: Option<crate::dispatch::RequestView>,
}

/// One stop of the trip under way.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StopLine {
    pub time: String,
    pub name: String,
    /// 0 already served, 1 the next stop, 2 still ahead.
    pub mark: u8,
}

/// One bus on the depot page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct YardBus {
    pub nr: String,
    pub name: String,
    pub condition: f32,
    pub km: String,
    pub place: String,
    /// Waiting in the depot, not out on a line.
    pub home: bool,
    pub yours: bool,
    pub workshop: bool,
}

/// One line from the company for the control room.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeskNote {
    pub level: u8,
    pub text: String,
}

/// One trip, as the timetable has it, before it becomes a row.
#[derive(Clone, Debug)]
pub(crate) struct TripIn {
    pub departure: f64,
    pub end: f64,
    pub line: String,
    pub from: String,
    pub to: String,
    pub break_row: bool,
}

/// A trip's times, for [`row_states`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub departure: f64,
    pub end: f64,
    pub is_break: bool,
}

/// The state of each trip at `now` (seconds of the day).
///
/// A trip that has ended is done. The one that is running is "now". The first that has
/// not started is "next" (a break stays a break). The rest are planned.
pub(crate) fn row_states(spans: &[Span], now: f64) -> Vec<RowState> {
    let mut out = Vec::with_capacity(spans.len());
    let mut next_set = false;
    let mut now_set = false;
    for s in spans {
        if s.end <= now {
            out.push(RowState::Done);
            continue;
        }
        if !now_set && now >= s.departure && now < s.end {
            now_set = true;
            out.push(RowState::Now);
            continue;
        }
        if !next_set && s.departure >= now {
            next_set = true;
            out.push(if s.is_break { RowState::Break } else { RowState::Next });
            continue;
        }
        out.push(if s.is_break { RowState::Break } else { RowState::Planned });
    }
    out
}

/// Minutes from `now` until `at`. Already due is 0. A remainder under a minute still
/// counts as one, so the card does not say "in 0 min" while the departure is ahead.
pub(crate) fn minutes_until(now: f64, at: f64) -> i64 {
    let d = at - now;
    if d <= 0.0 {
        return 0;
    }
    ((d / 60.0).floor() as i64).max(1)
}

/// The first trip that has not started yet, and the minutes until it leaves.
pub(crate) fn next_departure(spans: &[(f64, f64)], now: f64) -> Option<(usize, i64)> {
    let i = spans.iter().position(|(dep, _)| *dep > now)?;
    Some((i, minutes_until(now, spans[i].0)))
}

/// How full the shift bar is: 0 at the start, 1 once the planned time is reached.
pub(crate) fn shift_progress(seconds: f64, planned: f64) -> f32 {
    if !planned.is_finite() || planned <= 0.0 || !seconds.is_finite() {
        return 0.0;
    }
    (seconds.max(0.0) / planned).clamp(0.0, 1.0) as f32
}

/// Which rows fit: one before the focused line, then the next ones. The third number
/// is how many trips stay hidden under the list.
pub(crate) fn rows_in_view(n: usize, focus: usize, fit: usize) -> (usize, usize, usize) {
    if n == 0 || fit == 0 {
        return (0, 0, 0);
    }
    if n <= fit {
        return (0, n, 0);
    }
    let start = focus.min(n - 1).saturating_sub(1).min(n - fit);
    let end = (start + fit).min(n);
    (start, end, n - end)
}

/// Share of stops that were not late, or none when no stop was served.
pub(crate) fn punctual_share(on_time: i32, total: i32) -> Option<f32> {
    if total <= 0 {
        None
    } else {
        Some((on_time.max(0) as f32 / total as f32).clamp(0.0, 1.0))
    }
}

/// "136" and "04" become "136-04". Empty is empty (the caller says "free run").
pub(crate) fn duty_title(line: &str, tour: &str) -> String {
    let line = line.trim();
    let tour = tour.trim();
    if line.is_empty() && tour.is_empty() {
        String::new()
    } else if tour.is_empty() || tour == line {
        line.to_string()
    } else if tour.starts_with(line) {
        tour.to_string()
    } else {
        format!("{line}-{tour}")
    }
}

/// A pause row: the timetable named it that way.
pub(crate) fn is_break_label(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("pause") || t.contains("break")
}

/// "HH:MM" into seconds of the day.
pub(crate) fn parse_hm(text: &str) -> Option<f64> {
    let (h, m) = text.trim().split_once(':')?;
    let h: f64 = h.trim().parse().ok()?;
    let m: f64 = m.trim().parse().ok()?;
    (h.is_finite() && m.is_finite()).then_some(h * 3600.0 + m * 60.0)
}

/// Euros with cents, grouped as the launcher groups whole euros ("1.240,00 €").
pub(crate) fn euro(v: f64) -> String {
    let cents = if v.is_finite() { (v * 100.0).round() as i64 } else { 0 };
    let neg = cents < 0;
    let cents = cents.unsigned_abs();
    let whole = (cents / 100).to_string();
    let frac = cents % 100;
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!("{}{grouped},{frac:02} €", if neg { "−" } else { "" })
}

/// The shift ticket's lines at `now`.
pub(crate) fn duty_rows(trips: &[TripIn], now: f64) -> Vec<DutyRow> {
    let spans: Vec<Span> = trips
        .iter()
        .map(|t| Span {
            departure: t.departure,
            end: t.end,
            is_break: t.break_row || is_break_label(&t.from) || is_break_label(&t.to),
        })
        .collect();
    let states = row_states(&spans, now);
    trips
        .iter()
        .zip(states)
        .map(|(t, state)| DutyRow {
            time: crate::schedule::hhmm(t.departure),
            line: t.line.clone(),
            what: t.from.clone(),
            detail: t.to.clone(),
            arrival: crate::schedule::hhmm(t.end),
            state,
        })
        .collect()
}

/// "Good morning", "Good afternoon" or "Good evening" from the time of day.
pub(crate) fn day_hello(seconds_of_day: f64) -> &'static str {
    let h = if seconds_of_day.is_finite() { (seconds_of_day / 3600.0).floor() as i32 } else { 12 };
    let h = h.rem_euclid(24);
    if (5..12).contains(&h) {
        "Good morning"
    } else if (12..18).contains(&h) {
        "Good afternoon"
    } else {
        "Good evening"
    }
}

/// The clock-in card: first departure to last arrival, how many trips and pauses.
pub(crate) fn duty_facts(trips: &[TripIn]) -> Option<DutyFacts> {
    let first = trips.first()?;
    let last = trips.last()?;
    let mut n = 0usize;
    let mut breaks = 0usize;
    let mut break_sec = 0.0;
    for t in trips {
        let pause = t.break_row || is_break_label(&t.from) || is_break_label(&t.to);
        if pause {
            breaks += 1;
            break_sec += (t.end - t.departure).max(0.0);
        } else {
            n += 1;
        }
    }
    let dest = if first.to.trim().is_empty() { last.to.trim() } else { first.to.trim() };
    let route = match (first.from.trim(), dest) {
        ("", "") => String::new(),
        (a, "") => a.to_string(),
        ("", b) => b.to_string(),
        (a, b) if a.eq_ignore_ascii_case(b) => a.to_string(),
        (a, b) => format!("{a} ↔ {b}"),
    };
    Some(DutyFacts {
        start: crate::schedule::hhmm(first.departure),
        end: crate::schedule::hhmm(last.end),
        line: first.line.clone(),
        route,
        trips: n,
        breaks,
        break_min: (break_sec / 60.0).round() as i64,
        first: crate::schedule::hhmm(first.departure),
    })
}

/// Six bays, a stable one for this fleet number. Not a place on the map.
pub(crate) fn stall_of(nr: &str) -> u8 {
    let digits: String = nr.chars().filter(|c| c.is_ascii_digit()).collect();
    let n: u32 = digits.parse().unwrap_or(0);
    (n % 6) as u8
}

/// Kilometres, stops and crashes since `snap`.
pub(crate) fn bill_from(metres: f64, stops: [i32; 3], crashes: i32, snap: ShiftSnap, seconds: f64) -> ShiftBill {
    let served = (stops[0] - snap.stops[0]).max(0);
    let late = (stops[2] - snap.stops[2]).max(0);
    ShiftBill {
        seconds: if seconds.is_finite() { seconds.max(0.0) } else { 0.0 },
        km: if metres.is_finite() { ((metres - snap.metres) / 1000.0).max(0.0) } else { 0.0 },
        on_time: (served - late).max(0),
        stops: served,
        crashes: (crashes - snap.crashes).max(0),
    }
}

/// 0 when the stop is already served, 1 when it is the next one, 2 when it is still ahead.
pub(crate) fn stop_mark(index: usize, next: usize) -> u8 {
    if index < next { 0 } else if index == next { 1 } else { 2 }
}

/// Sort key for the depot list: the bus you have first, then the fleet number.
pub(crate) fn yard_key(yours: bool, nr: &str) -> (u8, u32) {
    let digits: String = nr.chars().filter(|c| c.is_ascii_digit()).collect();
    (if yours { 0 } else { 1 }, digits.parse().unwrap_or(0))
}

/// Regular pay and the overtime part, both from `shift_pay` (the overtime part is the
/// difference, so a new bonus cannot appear here).
pub(crate) fn wage_parts(seconds: f64, planned: f64, hourly: f64, extra: f64) -> (f64, f64, f64, f64, f64) {
    use omsi_launcher_lib::company::shift_pay;
    let all = shift_pay(seconds, planned, hourly, extra);
    let cap = if planned.is_finite() { seconds.max(0.0).min(planned.max(0.0)) } else { seconds.max(0.0) };
    let reg = shift_pay(cap, planned, hourly, extra);
    let overtime_pay = (all.wage - reg.wage).max(0.0);
    (reg.regular_hours, all.overtime_hours, reg.wage, overtime_pay, all.wage)
}

/// Hours and minutes of a span, as the shift card shows the plan ("0:20", "8:00").
pub(crate) fn hm_span(seconds: f64) -> String {
    let s = if seconds.is_finite() { seconds.max(0.0) as u64 } else { 0 };
    format!("{}:{:02}", s / 3600, (s % 3600) / 60)
}

/// The tablet for this frame.
pub(crate) fn tablet_view(app: &App) -> TabletView {
    use omsi_launcher_lib::company::{shift_pay, tuning, COLORS};
    let t = tuning();
    let planned = t.shift_minutes * 60.0;
    let seconds = if app.shift_on { app.shift_seconds } else { 0.0 };
    let pay = shift_pay(seconds, planned, t.driver_wage, t.overtime_extra);
    let full = shift_pay(planned, planned, t.driver_wage, t.overtime_extra);
    let (short, color_i) = if let Some(c) = &app.company {
        let short = if c.short.trim().is_empty() { c.name.clone() } else { c.short.clone() };
        (short, Some(c.color))
    } else if let Some((name, _)) = &app.remote_company {
        (name.clone(), None)
    } else {
        (String::new(), None)
    };
    let hex = color_i.and_then(|i| COLORS.get(i).map(|c| c.1)).unwrap_or(0xF2C2_30);
    let color = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255];
    let driver = app
        .career
        .driver
        .as_ref()
        .map(|d| d.name.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| app.career.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()))
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let n = app.args.lan_name.trim();
            (!n.is_empty()).then(|| n.to_string())
        })
        .unwrap_or_else(|| omsi_ui::tr("Driver").into_owned());
    let trips = app.duty.as_ref().map(|d| trip_ins(d)).unwrap_or_default();
    let rows = duty_rows(&trips, app.clock.time);
    let free_run = rows.is_empty();
    let duty_name = app.duty.as_ref().map(|d| duty_title(&d.line, &d.tour)).filter(|s| !s.is_empty()).unwrap_or_else(|| omsi_ui::tr("Free run").into_owned());
    let spans: Vec<(f64, f64)> = trips.iter().map(|t| (t.departure, t.end)).collect();
    let next = next_departure(&spans, app.clock.time).map(|(i, minutes)| NextLeave {
        time: crate::schedule::hhmm(trips[i].departure),
        line: trips[i].line.clone(),
        dest: trips[i].to.clone(),
        from: trips[i].from.clone(),
        minutes,
    });
    let start_s = trips.first().map(|t| t.departure).or_else(|| parse_hm(&app.args.time));
    let (shift_start, shift_end) = match start_s {
        Some(s) => (crate::schedule::hhmm(s), crate::schedule::hhmm(s + planned)),
        None => (String::new(), String::new()),
    };
    let (bus_name, bus_nr, condition, tank, km) = bus_of(app);
    let total = app.career.stops[0];
    let on_time = (total - app.career.stops[2]).max(0);
    let punctual = (total > 0).then_some((on_time, total));
    let (day, month) = app.clock.day_month();
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let date = format!("{} {:02}.{:02}.", omsi_ui::tr(days[app.clock.weekday().rem_euclid(7) as usize]), day, month);
    let facts = duty_facts(&trips);
    let tour_label = app
        .duty
        .as_ref()
        .map(|d| d.tour.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| omsi_ui::tr("Run {n}").replace("{n}", &t))
        .unwrap_or_default();
    let depot = app.company.as_ref().map(|c| c.depot.trim().to_string()).unwrap_or_default();
    let stall = (!bus_nr.is_empty()).then(|| stall_of(&bus_nr));
    let mode = if app.tablet_after {
        TabletMode::Settlement
    } else if app.shift_on {
        TabletMode::OnDuty
    } else {
        TabletMode::ClockIn
    };
    let bill = if app.tablet_after && !app.shift_on {
        app.shift_bill
    } else if app.tablet_after && app.shift_on {
        let snap = app.shift_snap.unwrap_or(ShiftSnap { metres: app.career.metres, stops: app.career.stops, crashes: app.career.crashes[0] });
        Some(bill_from(app.career.metres, app.career.stops, app.career.crashes[0], snap, app.shift_seconds))
    } else {
        None
    };
    let (stop_title, stops) = duty_stops(app);
    let bus_line = bus_line_of(app);
    let yard = yard_buses(app);
    let yard_home = yard.iter().filter(|b| b.home).count();
    let (next_stop, next_stop_time) = next_stop_of(app);
    let notes = desk_notes(app);
    let slip = bill.map(|b| {
        let (reg_h, ot_h, reg_pay, ot_pay, wage) = wage_parts(b.seconds, planned, t.driver_wage, t.overtime_extra);
        let pct = (t.overtime_extra.max(0.0) * 100.0).round() as i64;
        let overtime_line = if ot_pay > 0.004 {
            omsi_ui::tr("Overtime {time} h (+{pct} %)").replace("{time}", &hm_span(ot_h * 3600.0)).replace("{pct}", &pct.to_string())
        } else {
            String::new()
        };
        Slip {
            time: hm_span(b.seconds),
            overtime: if ot_h > 0.0 { hm_span(ot_h * 3600.0) } else { String::new() },
            km: format!("{} km", km_text(b.km)),
            trips: facts.as_ref().map(|f| f.trips).unwrap_or(0),
            punctual: (b.stops > 0).then_some((b.on_time, b.stops)),
            regular: omsi_ui::tr("{time} h × {wage}").replace("{time}", &hm_span(reg_h * 3600.0)).replace("{wage}", &euro(t.driver_wage)),
            regular_pay: euro(reg_pay),
            overtime_line,
            overtime_pay: euro(ot_pay),
            crashes: b.crashes,
            wage: euro(wage),
        }
    });
    TabletView {
        mode,
        tab: app.tablet_tab.min(4),
        hello: omsi_ui::tr(day_hello(app.clock.time)).into_owned(),
        facts,
        tour_label,
        depot,
        stall,
        slip,
        clock: crate::schedule::hhmm(app.clock.time),
        date,
        short,
        color,
        driver,
        duty_name,
        free_run,
        duty_clock: hm_clock(seconds),
        planned_label: hm_span(planned),
        progress: shift_progress(seconds, planned),
        overtime: app.shift_on && seconds > planned,
        shift_start,
        shift_end,
        wage_now: euro(pay.wage),
        hourly: euro(t.driver_wage),
        wage_full: euro(full.wage),
        overtime_pct: format!("+{:.0} %", t.overtime_extra.max(0.0) * 100.0),
        bus_name,
        bus_nr,
        condition,
        tank,
        km,
        punctual,
        next,
        rows,
        stops,
        stop_title,
        bus_line,
        yard_home,
        yard,
        fleet_remote: app.company.is_none() && app.remote_company.is_some(),
        next_stop,
        next_stop_time,
        notes,
        board: if app.tablet_tab == 4 { crate::dispatch::board_view(app) } else { None },
        request: crate::dispatch::request_view(app),
    }
}

fn duty_stops(app: &App) -> (String, Vec<StopLine>) {
    let Some(d) = app.duty.as_ref() else { return (String::new(), Vec::new()) };
    let Some(trip) = d.trips.get(d.trip_index) else { return (String::new(), Vec::new()) };
    let line = if trip.line.trim().is_empty() { d.line.trim() } else { trip.line.trim() };
    let title = if trip.terminus.trim().is_empty() { line.to_string() } else { format!("{line} → {}", trip.terminus.trim()) };
    let last = trip.stops.iter().rposition(|s| s.stops);
    let rows = trip
        .stops
        .iter()
        .enumerate()
        .filter(|(_, s)| s.stops && !s.name.trim().is_empty())
        .map(|(k, s)| StopLine {
            time: crate::schedule::hhmm(if Some(k) == last { s.arr } else { s.dep }),
            name: s.name.trim().to_string(),
            mark: stop_mark(k, d.next_stop),
        })
        .collect();
    (title, rows)
}

fn next_stop_of(app: &App) -> (String, String) {
    let Some(d) = app.duty.as_ref() else { return (String::new(), String::new()) };
    let Some(s) = d.trips.get(d.trip_index).and_then(|t| t.stops.get(d.next_stop)) else { return (String::new(), String::new()) };
    (s.name.trim().to_string(), crate::schedule::hhmm(s.dep))
}

fn bus_line_of(app: &App) -> String {
    let Some(v) = shown_vehicle(app) else { return String::new() };
    let file = v.ty.def.path.to_string_lossy();
    app.company
        .as_ref()
        .and_then(|c| c.buses.iter().find(|b| omsi_launcher_lib::company::same_file(&b.file, &file)))
        .map(|b| b.line.trim().to_string())
        .unwrap_or_default()
}

fn yard_buses(app: &App) -> Vec<YardBus> {
    let Some(c) = &app.company else { return Vec::new() };
    let file = shown_vehicle(app).map(|v| v.ty.def.path.to_string_lossy().into_owned()).unwrap_or_default();
    let mut yard: Vec<YardBus> = c
        .buses
        .iter()
        .map(|b| {
            let home = b.line.trim().is_empty();
            YardBus {
                nr: b.nr.to_string(),
                name: b.name.clone(),
                condition: (b.condition / 100.0).clamp(0.0, 1.0) as f32,
                km: format!("{} km", km_text(b.km)),
                place: if b.condition < omsi_launcher_lib::company::WORKSHOP_BELOW {
                    omsi_ui::tr("Workshop").into_owned()
                } else if home {
                    omsi_ui::tr("In the depot").into_owned()
                } else {
                    format!("{} {}", omsi_ui::tr("Line"), b.line.trim())
                },
                home,
                yours: !file.is_empty() && omsi_launcher_lib::company::same_file(&b.file, &file),
                workshop: b.condition < omsi_launcher_lib::company::WORKSHOP_BELOW,
            }
        })
        .collect();
    yard.sort_by_key(|b| yard_key(b.yours, &b.nr));
    yard
}

fn desk_notes(app: &App) -> Vec<DeskNote> {
    let Some(c) = &app.company else {
        if app.remote_company.is_some() {
            return vec![DeskNote { level: 0, text: omsi_ui::tr("The fleet is kept by the host.").into_owned() }];
        }
        return Vec::new();
    };
    c.notices(0)
        .into_iter()
        .filter(|( _, tmpl, _)| !(app.as_driver && tmpl.contains("account")))
        .map(|(level, tmpl, val)| DeskNote { level, text: omsi_ui::tr(tmpl).replace("{}", &val) })
        .collect()
}

fn hm_clock(seconds: f64) -> String {
    let s = if seconds.is_finite() { seconds.max(0.0) as u64 } else { 0 };
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

fn trip_ins(d: &crate::schedule::PlayerDuty) -> Vec<TripIn> {
    d.trips
        .iter()
        .map(|t| {
            let from = t.stops.iter().find(|s| !s.name.trim().is_empty()).map(|s| s.name.trim().to_string()).unwrap_or_else(|| t.name.trim().to_string());
            let to = t.terminus.trim().to_string();
            TripIn {
                departure: t.departure,
                end: t.end.max(t.departure),
                line: if t.line.trim().is_empty() { d.line.clone() } else { t.line.clone() },
                break_row: is_break_label(&t.name) || is_break_label(&from) || is_break_label(&to),
                from,
                to,
            }
        })
        .collect()
}

/// The name on the vehicle card: manufacturer and type, else the file name.
pub(crate) fn bus_title(manufacturer: &str, type_name: &str, stem: &str) -> String {
    let both = format!("{} {}", manufacturer.trim(), type_name.trim());
    let both = both.trim();
    if !both.is_empty() {
        both.to_string()
    } else {
        stem.trim().to_string()
    }
}

/// Kilometres as the card shows them: "12,4" under 100, else grouped whole kilometres ("66.985").
pub(crate) fn km_text(km: f64) -> String {
    let km = if km.is_finite() { km.max(0.0) } else { 0.0 };
    if km < 100.0 {
        let t = (km * 10.0).round() / 10.0;
        let whole = t.floor() as u64;
        let frac = ((t - whole as f64) * 10.0).round() as u64;
        let (whole, frac) = if frac >= 10 { (whole + 1, 0) } else { (whole, frac) };
        return format!("{whole},{frac}");
    }
    let mut grouped = String::new();
    let s = km.round().to_string();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    grouped
}

/// The bus on the card: the one driven, or the one parked when the player is on foot.
fn shown_vehicle(app: &App) -> Option<&omsi_sim::VehicleInstance> {
    if let Some(p) = app.player.as_ref() {
        return Some(&p.vehicle);
    }
    let want = app.driven_bus_file();
    app.placed
        .iter()
        .rev()
        .find(|p| omsi_launcher_lib::company::same_file(&p.vehicle.ty.def.path.to_string_lossy(), &want))
        .or_else(|| app.placed.last())
        .map(|p| &p.vehicle)
}

fn bus_of(app: &App) -> (String, String, Option<f32>, Option<f32>, Option<f64>) {
    let Some(v) = shown_vehicle(app) else {
        return (String::new(), String::new(), None, None, None);
    };
    let def = &v.ty.def;
    let stem = def.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = bus_title(&def.manufacturer, &def.type_name, &stem);
    let file = v.ty.def.path.to_string_lossy();
    let owned = app.company.as_ref().and_then(|c| c.buses.iter().find(|b| omsi_launcher_lib::company::same_file(&b.file, &file)));
    let nr = owned.map(|b| b.nr.to_string()).filter(|n| n != "0").or_else(|| {
        let n = v.number().trim().to_string();
        (!n.is_empty()).then_some(n)
    }).unwrap_or_default();
    let condition = owned.map(|b| (b.condition / 100.0).clamp(0.0, 1.0) as f32);
    let tank = v.var("tank_percent").filter(|t| t.is_finite()).map(|t| if t > 1.5 { t / 100.0 } else { t }).map(|t| t.clamp(0.0, 1.0));
    let km = match v.var("kmcounter_km").filter(|k| k.is_finite()) {
        Some(km) => Some(km as f64 + v.var("kmcounter_m").filter(|m| m.is_finite()).unwrap_or(0.0) as f64 / 1000.0),
        None => owned.map(|b| b.km).filter(|k| *k > 0.0),
    };
    (name, nr, condition, tank, km)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(dep_h: f64, dep_m: f64, end_h: f64, end_m: f64, is_break: bool) -> Span {
        Span { departure: dep_h * 3600.0 + dep_m * 60.0, end: end_h * 3600.0 + end_m * 60.0, is_break }
    }

    fn trip(dep: f64, end: f64, line: &str, from: &str, to: &str) -> TripIn {
        TripIn { departure: dep, end, line: line.into(), from: from.into(), to: to.into(), break_row: false }
    }

    #[test]
    fn states_before_during_and_after_the_duty_and_an_empty_one() {
        let spans = [span(9.0, 0.0, 10.0, 0.0, false), span(10.0, 0.0, 11.0, 0.0, false), span(11.0, 0.0, 11.0, 20.0, true)];
        assert!(row_states(&[], 9.0 * 3600.0).is_empty());
        assert_eq!(row_states(&spans, 8.0 * 3600.0), vec![RowState::Next, RowState::Planned, RowState::Break]);
        assert_eq!(row_states(&spans, 9.0 * 3600.0 + 30.0 * 60.0), vec![RowState::Now, RowState::Next, RowState::Break]);
        assert_eq!(row_states(&spans, 12.0 * 3600.0), vec![RowState::Done, RowState::Done, RowState::Done]);
    }

    #[test]
    fn the_next_departure_is_the_first_that_has_not_started() {
        let spans = [(9.0 * 3600.0, 10.0 * 3600.0), (10.0 * 3600.0, 11.0 * 3600.0)];
        assert_eq!(next_departure(&spans, 8.0 * 3600.0 + 50.0 * 60.0), Some((0, 10)));
        assert_eq!(next_departure(&spans, 9.0 * 3600.0 + 30.0 * 60.0), Some((1, 30)));
        assert_eq!(next_departure(&spans, 11.0 * 3600.0), None);
        assert_eq!(minutes_until(100.0, 100.0), 0);
        assert_eq!(minutes_until(100.0, 130.0), 1);
    }

    #[test]
    fn the_shift_bar_fills_up_to_the_planned_time() {
        assert_eq!(shift_progress(0.0, 20.0 * 60.0), 0.0);
        assert!((shift_progress(10.0 * 60.0, 20.0 * 60.0) - 0.5).abs() < 1e-4);
        assert_eq!(shift_progress(40.0 * 60.0, 20.0 * 60.0), 1.0);
        assert_eq!(shift_progress(10.0, 0.0), 0.0);
    }

    #[test]
    fn the_list_keeps_the_current_trip_and_counts_the_rest() {
        assert_eq!(rows_in_view(0, 0, 4), (0, 0, 0));
        assert_eq!(rows_in_view(3, 1, 8), (0, 3, 0));
        assert_eq!(rows_in_view(10, 5, 4), (4, 8, 2));
        assert_eq!(rows_in_view(10, 0, 4), (0, 4, 6));
    }

    #[test]
    fn rows_carry_the_time_and_the_state() {
        let now = 9.0 * 3600.0 + 30.0 * 60.0;
        let rows = duty_rows(
            &[trip(9.0 * 3600.0, 10.0 * 3600.0, "136", "Rathaus", "Herringdorf"), trip(10.0 * 3600.0, 11.0 * 3600.0, "136", "Herringdorf", "Rathaus")],
            now,
        );
        assert_eq!(rows[0].time, "09:00");
        assert_eq!(rows[0].arrival, "10:00");
        assert_eq!(rows[0].state, RowState::Now);
        assert_eq!(rows[1].state, RowState::Next);
        assert!(duty_rows(&[], now).is_empty());
    }

    #[test]
    fn money_titles_and_a_clock_time() {
        assert_eq!(euro(5.1), "5,10 €");
        assert_eq!(euro(3.666), "3,67 €");
        assert_eq!(euro(1240.0), "1.240,00 €");
        assert_eq!(duty_title("136", "04"), "136-04");
        assert_eq!(duty_title("", ""), "");
        assert_eq!(duty_title("136", "136-04"), "136-04");
        assert_eq!(parse_hm("09:12"), Some(9.0 * 3600.0 + 12.0 * 60.0));
        assert_eq!(parse_hm("nope"), None);
        assert!(is_break_label("Pause (20 min)"));
        assert_eq!(punctual_share(12, 13).map(|p| (p * 100.0).round() as i32), Some(92));
        assert_eq!(punctual_share(0, 0), None);
        assert_eq!(hm_span(20.0 * 60.0), "0:20");
        assert_eq!(hm_span(8.0 * 3600.0), "8:00");
        assert_eq!(bus_title("Mercedes", "Citaro", "citaro"), "Mercedes Citaro");
        assert_eq!(bus_title("", "", "citaro"), "citaro");
        assert_eq!(km_text(12.36), "12,4");
        assert_eq!(km_text(66985.2), "66.985");
    }

    #[test]
    fn greeting_duty_card_stall_and_the_wage_split() {
        assert_eq!(day_hello(8.0 * 3600.0), "Good morning");
        assert_eq!(day_hello(14.0 * 3600.0), "Good afternoon");
        assert_eq!(day_hello(20.0 * 3600.0), "Good evening");
        let facts = duty_facts(&[
            trip(8.0 * 3600.0 + 45.0 * 60.0, 9.0 * 3600.0 + 30.0 * 60.0, "136", "Rathaus", "Hennigsdorf"),
            trip(10.0 * 3600.0, 10.0 * 3600.0 + 20.0 * 60.0, "136", "Pause", "Pause"),
            trip(15.0 * 3600.0, 17.0 * 3600.0 + 15.0 * 60.0, "136", "Hennigsdorf", "Rathaus"),
        ])
        .unwrap();
        assert_eq!(facts.start, "08:45");
        assert_eq!(facts.end, "17:15");
        assert_eq!(facts.first, "08:45");
        assert_eq!(facts.trips, 2);
        assert_eq!(facts.breaks, 1);
        assert_eq!(facts.break_min, 20);
        assert_eq!(facts.route, "Rathaus ↔ Hennigsdorf");
        assert!(duty_facts(&[]).is_none());
        assert_eq!(stop_mark(0, 2), 0);
        assert_eq!(stop_mark(2, 2), 1);
        assert_eq!(stop_mark(4, 2), 2);
        assert!(yard_key(true, "10") < yard_key(false, "1"));
        assert!(yard_key(false, "2") < yard_key(false, "1003"));
        assert_eq!(stall_of("1003"), 1);
        assert_eq!(stall_of(""), 0);
        let (reg_h, ot_h, reg_pay, ot_pay, wage) = wage_parts(30.0 * 60.0, 20.0 * 60.0, 22.0, 0.25);
        assert!((reg_h - 20.0 / 60.0).abs() < 1e-6);
        assert!((ot_h - 10.0 / 60.0).abs() < 1e-6);
        assert!((reg_pay - 7.33).abs() < 0.001);
        assert!((ot_pay - 4.59).abs() < 0.001);
        assert!((wage - 11.92).abs() < 0.001);
        let snap = ShiftSnap { metres: 1000.0, stops: [4, 1, 1], crashes: 2 };
        let bill = bill_from(3500.0, [10, 2, 3], 2, snap, 600.0);
        assert!((bill.km - 2.5).abs() < 1e-6);
        assert_eq!(bill.stops, 6);
        assert_eq!(bill.on_time, 4);
        assert_eq!(bill.crashes, 0);
    }
}
