//! The dispatcher in multiplayer: who drives which duty.
//!
//! The host is the boss. It may name one of the others the dispatcher; without one the boss
//! hands out the duties himself (and may always do so). The duties are the tours of the
//! timetable, the company's lines first. Handing one out asks that player on his tablet; he
//! takes it or says no, and a duty taken starts in his game as one picked from the menu.
//!
//! The host keeps the plan and sends it to everybody every few seconds (`dispo plan ...`),
//! with the dispatcher's id (`dispo role ...`) and the company's lines (`dispo lines ...`).
//! The dispatcher's game asks the host (`dispo give ...`, `dispo free ...`); the host asks the
//! driver (`dispo ask ...`), who answers (`dispo answer ...`). Fields are parted by `¦` (a
//! command's text may hold neither `|` nor control characters).

use crate::app::App;

/// Between the fields of a command.
pub(crate) const SEP: char = '¦';
/// Seconds between the host's plan announcements.
const ANNOUNCE_EVERY: f32 = 5.0;
/// Rows of the duty list the up and down buttons move.
pub(crate) const SCROLL_STEP: usize = 4;

/// Where a duty handed out stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssignState {
    /// Asked, no answer yet.
    Asked,
    /// Taken by the driver.
    Taken,
    /// The driver said no.
    Declined,
}

impl AssignState {
    fn code(self) -> &'static str {
        match self {
            AssignState::Asked => "a",
            AssignState::Taken => "y",
            AssignState::Declined => "n",
        }
    }

    fn from_code(s: &str) -> Option<AssignState> {
        match s.trim() {
            "a" => Some(AssignState::Asked),
            "y" => Some(AssignState::Taken),
            "n" => Some(AssignState::Declined),
            _ => None,
        }
    }
}

/// A duty handed out.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Assign {
    pub line: String,
    pub tour: String,
    pub pid: u32,
    pub name: String,
    pub state: AssignState,
}

/// A duty this player was asked to drive.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Request {
    pub line: String,
    pub tour: String,
    pub start: f64,
    pub end: f64,
    pub by: String,
}

/// The dispatcher's state, in every game of a session (the host's is the one that counts).
#[derive(Clone, Debug, Default)]
pub(crate) struct Dispatch {
    /// The player named dispatcher.
    pub dispatcher: Option<u32>,
    /// The company's lines, shown first (the host's company; empty: all lines).
    pub lines: Vec<String>,
    pub plan: Vec<Assign>,
    /// The plan's version: the host counts it up with every change; the others take a newer
    /// one whole.
    pub gen: u64,
    /// A duty this player was asked to drive, waiting for an answer.
    pub request: Option<Request>,
    /// The board: the line shown, the tour picked, the first row shown.
    pub line: Option<String>,
    pub picked: Option<String>,
    pub scroll: usize,
    sync_t: f32,
}

/// One tour of the timetable, as the board lists it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DutySlot {
    pub line: String,
    pub tour: String,
    pub start: f64,
    pub end: f64,
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The fields of a command's argument.
fn fields(arg: &str) -> Vec<&str> {
    arg.split(SEP).collect()
}

/// The plan as commands (`plan` and its fields; the caller puts `dispo ` before each).
pub(crate) fn encode_plan(gen: u64, plan: &[Assign]) -> Vec<String> {
    if plan.is_empty() {
        return vec![format!("plan {gen}{SEP}0")];
    }
    plan.iter()
        .map(|a| {
            let name: String = a.name.chars().filter(|c| *c != SEP).take(32).collect();
            format!("plan {gen}{SEP}{}{SEP}{}{SEP}{}{SEP}{}{SEP}{}{SEP}{name}", plan.len(), a.line.trim(), a.tour.trim(), a.pid, a.state.code())
        })
        .collect()
}

/// Take a `plan` command's argument: a newer plan replaces the one known; the same one is
/// filled up. True when the plan changed.
pub(crate) fn apply_plan(d: &mut Dispatch, arg: &str) -> bool {
    let f = fields(arg);
    let Some(gen) = f.first().and_then(|g| g.trim().parse::<u64>().ok()) else { return false };
    if gen < d.gen {
        return false;
    }
    let mut changed = false;
    if gen > d.gen {
        d.gen = gen;
        changed = !d.plan.is_empty();
        d.plan.clear();
    }
    if f.len() < 7 {
        return changed;
    }
    // (gen, count, line, tour, player, state, name)
    let (Some(pid), Some(state)) = (f[4].trim().parse::<u32>().ok(), AssignState::from_code(f[5])) else { return changed };
    let a = Assign { line: f[2].trim().to_string(), tour: f[3].trim().to_string(), pid, name: f[6..].join(&SEP.to_string()).trim().to_string(), state };
    match d.plan.iter_mut().find(|x| same(&x.line, &a.line) && same(&x.tour, &a.tour)) {
        Some(x) if *x == a => {}
        Some(x) => {
            *x = a;
            changed = true;
        }
        None => {
            d.plan.push(a);
            changed = true;
        }
    }
    changed
}

/// Hand duty `line`/`tour` to player `pid`: he is asked. A duty he had before is his no
/// longer (one duty a player).
pub(crate) fn give(plan: &mut Vec<Assign>, line: &str, tour: &str, pid: u32, name: &str) {
    plan.retain(|a| a.pid != pid || (same(&a.line, line) && same(&a.tour, tour)));
    let a = Assign { line: line.trim().to_string(), tour: tour.trim().to_string(), pid, name: name.trim().to_string(), state: AssignState::Asked };
    match plan.iter_mut().find(|x| same(&x.line, line) && same(&x.tour, tour)) {
        Some(x) => *x = a,
        None => plan.push(a),
    }
}

/// Nobody drives duty `line`/`tour` any more. True when somebody had it.
pub(crate) fn free(plan: &mut Vec<Assign>, line: &str, tour: &str) -> bool {
    let n = plan.len();
    plan.retain(|a| !(same(&a.line, line) && same(&a.tour, tour)));
    plan.len() != n
}

/// Player `pid` answers for duty `line`/`tour`: only the one asked can. True when it counted.
pub(crate) fn answer(plan: &mut [Assign], line: &str, tour: &str, pid: u32, yes: bool) -> bool {
    match plan.iter_mut().find(|a| same(&a.line, line) && same(&a.tour, tour) && a.pid == pid) {
        Some(a) => {
            a.state = if yes { AssignState::Taken } else { AssignState::Declined };
            true
        }
        None => false,
    }
}

/// The ask for a duty: line, tour, its times and who asks.
pub(crate) fn encode_ask(r: &Request) -> String {
    let by: String = r.by.chars().filter(|c| *c != SEP).take(32).collect();
    format!("ask {}{SEP}{}{SEP}{:.0}{SEP}{:.0}{SEP}{by}", r.line.trim(), r.tour.trim(), r.start, r.end)
}

pub(crate) fn parse_ask(arg: &str) -> Option<Request> {
    let f = fields(arg);
    if f.len() < 5 {
        return None;
    }
    let num = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0 && *v < 3.0 * 86400.0);
    let (line, tour) = (f[0].trim(), f[1].trim());
    if line.is_empty() || tour.is_empty() {
        return None;
    }
    Some(Request { line: line.to_string(), tour: tour.to_string(), start: num(f[2])?, end: num(f[3])?, by: f[4..].join(&SEP.to_string()).trim().to_string() })
}

/// "154-4" for the lists ("154" and "154-4" too).
pub(crate) fn duty_name(line: &str, tour: &str) -> String {
    crate::tablet::duty_title(line, tour)
}

/// The board's lines: the company's first (in their order), then the others.
pub(crate) fn board_lines(company: &[String], all: &[String]) -> Vec<String> {
    let mut out: Vec<String> = company.iter().filter(|l| all.iter().any(|a| same(a, l))).cloned().collect();
    if out.is_empty() {
        out = all.to_vec();
    }
    out
}

/// The first row to show so that `rows` fit, scrolled to `scroll` (kept in range).
pub(crate) fn clamp_scroll(scroll: usize, n: usize, fit: usize) -> usize {
    scroll.min(n.saturating_sub(fit.max(1)))
}

impl App {
    fn lan_role(&self) -> Option<omsi_net::Role> {
        self.lan.as_ref().filter(|l| l.connected || l.role == omsi_net::Role::Host).map(|l| l.role)
    }

    fn my_pid(&self) -> u32 {
        self.lan.as_ref().map(|l| l.my_id).unwrap_or(0)
    }

    /// This game may hand out duties: the host (the boss), or the dispatcher.
    pub(crate) fn can_dispatch(&self) -> bool {
        match self.lan_role() {
            Some(omsi_net::Role::Host) => true,
            Some(omsi_net::Role::Client) => self.dispo.dispatcher.is_some_and(|d| d == self.my_pid()),
            None => false,
        }
    }

    /// This game is the boss's (it names the dispatcher).
    pub(crate) fn is_chef(&self) -> bool {
        self.lan_role() == Some(omsi_net::Role::Host)
    }

    /// Everybody of the session: (id, name, line, tour), this game first.
    pub(crate) fn session_players(&self) -> Vec<(u32, String, String, String)> {
        let Some(l) = self.lan.as_ref() else { return Vec::new() };
        let me = if l.my_name.trim().is_empty() { omsi_ui::tr("Me").into_owned() } else { l.my_name.trim().to_string() };
        let (line, tour) = match self.duty.as_ref() {
            Some(_) => (self.args.line.clone().unwrap_or_default(), self.args.tour.clone().unwrap_or_default()),
            None => (String::new(), String::new()),
        };
        let mut out = vec![(l.my_id, me, line, tour)];
        let mut others: Vec<(u32, String, String, String)> = l
            .peers()
            .filter(|p| p.pose.id != l.my_id && p.pose.id != 0)
            .map(|p| {
                let name = if p.pose.name.trim().is_empty() { format!("{} {}", omsi_ui::tr("Player"), p.pose.id) } else { p.pose.name.trim().to_string() };
                (p.pose.id, name, p.pose.line.trim().to_string(), p.pose.tour.trim().to_string())
            })
            .collect();
        others.sort_by_key(|p| p.0);
        out.extend(others);
        out
    }

    fn player_name(&self, pid: u32) -> String {
        self.session_players().into_iter().find(|p| p.0 == pid).map(|p| p.1).unwrap_or_else(|| format!("{} {pid}", omsi_ui::tr("Player")))
    }

    /// The tours of the timetable the board lists, the company's lines first.
    pub(crate) fn duty_slots(&self) -> Vec<DutySlot> {
        let Some(sch) = self.schedule.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        for l in sch.data.lines.iter().filter(|l| l.user_allowed) {
            for t in l.tours.iter().filter(|t| sch.tour_available(t)) {
                let start = crate::game_lists::tour_start(t).unwrap_or(0.0);
                let end = sch.tour_stops(&l.name, &t.number).iter().map(|s| s.3).fold(start, f64::max);
                out.push(DutySlot { line: l.name.clone(), tour: t.number.trim().to_string(), start, end });
            }
        }
        out.sort_by(|a, b| a.start.total_cmp(&b.start));
        out
    }

    /// The lines on the board.
    pub(crate) fn dispatch_lines(&self) -> Vec<String> {
        let Some(sch) = self.schedule.as_ref() else { return Vec::new() };
        let mut all: Vec<String> = sch.data.lines.iter().filter(|l| l.user_allowed && l.tours.iter().any(|t| sch.tour_available(t))).map(|l| l.name.clone()).collect();
        all.sort_by(|a, b| {
            let key = |s: &str| (s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u64>().unwrap_or(u64::MAX), s.to_ascii_lowercase());
            key(a).cmp(&key(b))
        });
        let company: Vec<String> = match &self.company {
            Some(c) => c.lines.clone(),
            None => self.dispo.lines.clone(),
        };
        board_lines(&company, &all)
    }

    /// Every frame: the host tells the others the plan now and then.
    pub(crate) fn dispatch_tick(&mut self, dt: f32) {
        if self.lan_role() != Some(omsi_net::Role::Host) {
            return;
        }
        self.dispo.sync_t -= dt;
        if self.dispo.sync_t > 0.0 {
            return;
        }
        self.dispo.sync_t = ANNOUNCE_EVERY;
        let mut msgs = vec![format!("dispo role {}", self.dispo.dispatcher.unwrap_or(0))];
        if let Some(c) = &self.company {
            let lines: Vec<&str> = c.lines.iter().map(|s| s.trim()).collect();
            let mut text = format!("dispo lines {}", lines.join(&SEP.to_string()));
            while text.chars().count() > omsi_net::MAX_CHAT {
                text.pop();
            }
            msgs.push(text);
        }
        msgs.extend(encode_plan(self.dispo.gen, &self.dispo.plan).into_iter().map(|m| format!("dispo {m}")));
        let Some(l) = self.lan.as_mut() else { return };
        let ids: Vec<u32> = l.peers().map(|p| p.pose.id).filter(|id| *id != l.my_id && *id != 0).collect();
        for id in ids {
            for m in &msgs {
                l.command(id, m);
            }
        }
    }

    fn plan_changed(&mut self) {
        self.dispo.gen += 1;
        self.dispo.sync_t = 0.0;
    }

    /// A `dispo ...` command from player `from`.
    pub(crate) fn dispatch_command(&mut self, from: u32, text: &str) {
        let (verb, arg) = text.split_once(' ').unwrap_or((text, ""));
        let host = self.lan_role() == Some(omsi_net::Role::Host);
        match verb {
            // (host → us)
            "role" if !host && from == 1 => {
                let id = arg.trim().parse::<u32>().unwrap_or(0);
                let was = self.can_dispatch();
                self.dispo.dispatcher = (id != 0).then_some(id);
                if !was && self.can_dispatch() {
                    self.service_msg = Some((omsi_ui::tr("You are the dispatcher now: the control room is on the tablet (F8)").into_owned(), 8.0));
                } else if was && !self.can_dispatch() {
                    self.service_msg = Some((omsi_ui::tr("You are a driver again").into_owned(), 5.0));
                }
            }
            "lines" if !host && from == 1 => {
                self.dispo.lines = fields(arg).into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
            "plan" if !host && from == 1 => {
                apply_plan(&mut self.dispo, arg);
            }
            "ask" if !host && from == 1 => {
                if let Some(r) = parse_ask(arg) {
                    self.receive_request(r);
                }
            }
            // (dispatcher → host)
            "give" if host && Some(from) == self.dispo.dispatcher => {
                let f = fields(arg);
                if let (Some(line), Some(tour), Some(pid)) = (f.first(), f.get(1), f.get(2).and_then(|p| p.trim().parse::<u32>().ok())) {
                    self.host_give(line, tour, pid, from);
                }
            }
            "free" if host && Some(from) == self.dispo.dispatcher => {
                let f = fields(arg);
                if let (Some(line), Some(tour)) = (f.first(), f.get(1)) {
                    if free(&mut self.dispo.plan, line, tour) {
                        self.plan_changed();
                    }
                }
            }
            // (driver → host)
            "answer" if host => {
                let f = fields(arg);
                if let (Some(line), Some(tour), Some(yes)) = (f.first(), f.get(1), f.get(2)) {
                    self.host_answer(from, line, tour, yes.trim() == "1");
                }
            }
            _ => log::info!("dispatch: command '{verb}' from player {from} not taken"),
        }
    }

    fn receive_request(&mut self, r: Request) {
        let name = duty_name(&r.line, &r.tour);
        self.service_msg = Some((omsi_ui::tr("New duty from the control room: {duty}. The tablet (F8) asks you").replace("{duty}", &name), 10.0));
        self.dispo.request = Some(r);
    }

    /// The host hands out a duty (its own board, or the dispatcher `by` asked).
    fn host_give(&mut self, line: &str, tour: &str, pid: u32, by: u32) {
        let name = self.player_name(pid);
        give(&mut self.dispo.plan, line, tour, pid, &name);
        self.plan_changed();
        let slot = self.duty_slots().into_iter().find(|s| same(&s.line, line) && same(&s.tour, tour));
        let by = self.player_name(by);
        let r = Request { line: line.trim().to_string(), tour: tour.trim().to_string(), start: slot.as_ref().map(|s| s.start).unwrap_or(0.0), end: slot.as_ref().map(|s| s.end).unwrap_or(0.0), by };
        if pid == self.my_pid() {
            self.receive_request(r);
        } else if let Some(l) = self.lan.as_mut() {
            l.command(pid, &format!("dispo {}", encode_ask(&r)));
        }
    }

    fn host_answer(&mut self, pid: u32, line: &str, tour: &str, yes: bool) {
        if answer(&mut self.dispo.plan, line, tour, pid, yes) {
            self.plan_changed();
            let who = self.player_name(pid);
            let text = if yes { omsi_ui::tr("{who} takes duty {duty}") } else { omsi_ui::tr("{who} declines duty {duty}") };
            self.service_msg = Some((text.replace("{who}", &who).replace("{duty}", &duty_name(line, tour)), 6.0));
        }
    }

    /// The board hands duty `line`/`tour` to player `pid`.
    pub(crate) fn dispatch_give(&mut self, line: &str, tour: &str, pid: u32) {
        if !self.can_dispatch() {
            return;
        }
        if self.is_chef() {
            let me = self.my_pid();
            self.host_give(line, tour, pid, me);
        } else if let Some(l) = self.lan.as_mut() {
            l.command(1, &format!("dispo give {}{SEP}{}{SEP}{pid}", line.trim(), tour.trim()));
            // (shown at once; the host's next plan says how it stands)
            let name = self.player_name(pid);
            give(&mut self.dispo.plan, line, tour, pid, &name);
        }
    }

    /// The board takes duty `line`/`tour` back.
    pub(crate) fn dispatch_free(&mut self, line: &str, tour: &str) {
        if !self.can_dispatch() {
            return;
        }
        let had = free(&mut self.dispo.plan, line, tour);
        if self.is_chef() {
            if had {
                self.plan_changed();
            }
        } else if let Some(l) = self.lan.as_mut() {
            l.command(1, &format!("dispo free {}{SEP}{}", line.trim(), tour.trim()));
        }
    }

    /// The boss names player `pid` the dispatcher, or takes the role back from him.
    pub(crate) fn dispatch_role(&mut self, pid: u32) {
        if !self.is_chef() || pid == self.my_pid() {
            return;
        }
        self.dispo.dispatcher = if self.dispo.dispatcher == Some(pid) { None } else { Some(pid) };
        self.dispo.sync_t = 0.0;
        let text = match self.dispo.dispatcher {
            Some(id) => omsi_ui::tr("{who} is the dispatcher now").replace("{who}", &self.player_name(id)),
            None => omsi_ui::tr("No dispatcher: you hand out the duties").into_owned(),
        };
        self.service_msg = Some((text, 5.0));
    }

    /// This player answers the duty he was asked to drive. Taken, it starts at once.
    pub(crate) fn dispatch_answer(&mut self, yes: bool) {
        let Some(r) = self.dispo.request.take() else { return };
        if self.is_chef() {
            let me = self.my_pid();
            self.host_answer(me, &r.line, &r.tour, yes);
        } else if let Some(l) = self.lan.as_mut() {
            l.command(1, &format!("dispo answer {}{SEP}{}{SEP}{}", r.line, r.tour, yes as u8));
        }
        if yes {
            crate::game_lists::start_duty(self, &r.line, &r.tour);
            self.tablet_tab = 0;
        }
    }
}

/// A player on the board.
#[derive(Clone, Debug)]
pub(crate) struct BoardPlayer {
    pub id: u32,
    pub name: String,
    /// 0 the boss, 1 the dispatcher, 2 a driver.
    pub role: u8,
    /// The duty he drives now, or "free".
    pub doing: String,
    pub me: bool,
}

/// Where a duty of the board stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowMark {
    Open,
    Asked,
    Taken,
    Declined,
    /// Somebody drives it (taken from the menu, or handed out and started).
    Driving,
}

/// A duty on the board.
#[derive(Clone, Debug)]
pub(crate) struct BoardRow {
    pub tour: String,
    pub name: String,
    pub time: String,
    pub driver: String,
    pub mark: RowMark,
    pub picked: bool,
}

/// The duty picked on the board.
#[derive(Clone, Debug)]
pub(crate) struct BoardPick {
    pub name: String,
    pub time: String,
    /// Who has it (asked or taken), and how it stands.
    pub holder: Option<(u32, AssignState)>,
}

/// The control room of the boss and the dispatcher.
#[derive(Clone, Debug)]
pub(crate) struct BoardView {
    pub chef: bool,
    pub players: Vec<BoardPlayer>,
    pub lines: Vec<String>,
    pub line: String,
    pub rows: Vec<BoardRow>,
    pub scroll: usize,
    pub pick: Option<BoardPick>,
}

/// The ask on a driver's tablet.
#[derive(Clone, Debug)]
pub(crate) struct RequestView {
    pub duty: String,
    pub start: String,
    pub end: String,
    pub by: String,
}

/// The line the board shows: the one picked, else the first.
pub(crate) fn board_line(lines: &[String], picked: Option<&str>) -> Option<String> {
    picked.and_then(|p| lines.iter().find(|l| same(l, p)).cloned()).or_else(|| lines.first().cloned())
}

/// The control room for this frame (None: this game hands out no duties).
pub(crate) fn board_view(app: &App) -> Option<BoardView> {
    if !app.can_dispatch() {
        return None;
    }
    let players = app.session_players();
    let lines = app.dispatch_lines();
    let line = board_line(&lines, app.dispo.line.as_deref()).unwrap_or_default();
    let doing = |l: &str, t: &str| if l.trim().is_empty() { omsi_ui::tr("free").into_owned() } else { duty_name(l, t) };
    let board_players = players
        .iter()
        .map(|(id, name, l, t)| BoardPlayer {
            id: *id,
            name: name.clone(),
            role: if *id == 1 { 0 } else if app.dispo.dispatcher == Some(*id) { 1 } else { 2 },
            doing: doing(l, t),
            me: app.lan.as_ref().is_some_and(|x| x.my_id == *id),
        })
        .collect();
    let rows: Vec<BoardRow> = app
        .duty_slots()
        .into_iter()
        .filter(|d| same(&d.line, &line))
        .map(|d| {
            let held = app.dispo.plan.iter().find(|a| same(&a.line, &d.line) && same(&a.tour, &d.tour));
            let driving = players.iter().find(|p| same(&p.2, &d.line) && same(&p.3, &d.tour));
            let (driver, mark) = match (driving, held) {
                (Some(p), _) => (p.1.clone(), RowMark::Driving),
                (None, Some(a)) => (
                    a.name.clone(),
                    match a.state {
                        AssignState::Asked => RowMark::Asked,
                        AssignState::Taken => RowMark::Taken,
                        AssignState::Declined => RowMark::Declined,
                    },
                ),
                (None, None) => (String::new(), RowMark::Open),
            };
            BoardRow {
                name: duty_name(&d.line, &d.tour),
                time: format!("{}–{}", crate::schedule::hhmm(d.start), crate::schedule::hhmm(d.end)),
                picked: app.dispo.picked.as_deref().is_some_and(|p| same(p, &d.tour)),
                tour: d.tour,
                driver,
                mark,
            }
        })
        .collect();
    let pick = rows.iter().find(|r| r.picked).map(|r| BoardPick {
        name: r.name.clone(),
        time: r.time.clone(),
        holder: app.dispo.plan.iter().find(|a| same(&a.line, &line) && same(&a.tour, &r.tour)).map(|a| (a.pid, a.state)),
    });
    Some(BoardView { chef: app.is_chef(), players: board_players, lines, line, rows, scroll: app.dispo.scroll, pick })
}

/// The ask waiting on this player's tablet.
pub(crate) fn request_view(app: &App) -> Option<RequestView> {
    let r = app.dispo.request.as_ref()?;
    Some(RequestView { duty: duty_name(&r.line, &r.tour), start: crate::schedule::hhmm(r.start), end: crate::schedule::hhmm(r.end), by: r.by.clone() })
}

/// The tablet's lines for the control room (the ids its buttons answer to).
pub(crate) fn tablet_items(app: &App) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let tr = |t: &str| omsi_ui::tr(t).into_owned();
    let mut first = Vec::new();
    if app.handover.offer.is_some() {
        first.push((tr("Take the wheel"), "haccept".to_string()));
        first.push((tr("No, thanks"), "hdecline".to_string()));
    } else if app.dispo.request.is_some() {
        first.push((tr("Accept"), "daccept".to_string()));
        first.push((tr("Decline"), "ddecline".to_string()));
    }
    let mut rest = Vec::new();
    if app.tablet_tab == 2 {
        for (id, name) in app.riders_in_my_bus() {
            rest.push((name, format!("hgive {id}")));
        }
    }
    if app.tablet_tab == 4 {
        if let Some(b) = board_view(app) {
            for (i, l) in b.lines.iter().enumerate() {
                rest.push((format!("{} {l}", tr("Line")), format!("dline {i}")));
            }
            for (i, r) in b.rows.iter().enumerate() {
                rest.push((r.name.clone(), format!("drow {i}")));
            }
            if b.pick.is_some() {
                for p in &b.players {
                    rest.push((p.name.clone(), format!("dgive {}", p.id)));
                }
                rest.push((tr("Take back"), "dfree".to_string()));
            }
            rest.push(("▲".to_string(), "dup".to_string()));
            rest.push(("▼".to_string(), "ddown".to_string()));
            if b.chef {
                for p in b.players.iter().filter(|p| !p.me) {
                    rest.push((p.name.clone(), format!("drole {}", p.id)));
                }
            }
        }
    }
    (first, rest)
}

/// A control-room button of the tablet was used. True when it was one.
pub(crate) fn tablet_action(app: &mut App, verb: &str, arg: &str) -> bool {
    match verb {
        "haccept" => app.answer_wheel(true),
        "hdecline" => app.answer_wheel(false),
        "hgive" => {
            if let Ok(id) = arg.trim().parse::<u32>() {
                app.offer_wheel(id);
            }
        }
        "daccept" => app.dispatch_answer(true),
        "ddecline" => app.dispatch_answer(false),
        "dline" => {
            let lines = app.dispatch_lines();
            if let Some(l) = arg.trim().parse::<usize>().ok().and_then(|i| lines.get(i)) {
                app.dispo.line = Some(l.clone());
                app.dispo.picked = None;
                app.dispo.scroll = 0;
            }
        }
        "drow" => {
            if let Some(b) = board_view(app) {
                if let Some(r) = arg.trim().parse::<usize>().ok().and_then(|i| b.rows.get(i)) {
                    app.dispo.line = Some(b.line.clone());
                    app.dispo.picked = Some(r.tour.clone());
                }
            }
        }
        "dgive" | "dfree" => {
            let (Some(b), Some(tour)) = (board_view(app), app.dispo.picked.clone()) else { return true };
            if verb == "dfree" {
                app.dispatch_free(&b.line, &tour);
            } else if let Ok(pid) = arg.trim().parse::<u32>() {
                app.dispatch_give(&b.line, &tour, pid);
            }
        }
        "dup" => app.dispo.scroll = app.dispo.scroll.saturating_sub(SCROLL_STEP),
        "ddown" => {
            let n = board_view(app).map(|b| b.rows.len()).unwrap_or(0);
            app.dispo.scroll = clamp_scroll(app.dispo.scroll + SCROLL_STEP, n, 8);
        }
        "drole" => {
            if let Ok(pid) = arg.trim().parse::<u32>() {
                app.dispatch_role(pid);
            }
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(line: &str, tour: &str, pid: u32, state: AssignState) -> Assign {
        Assign { line: line.into(), tour: tour.into(), pid, name: format!("P{pid}"), state }
    }

    #[test]
    fn the_plan_goes_over_as_commands_and_a_newer_one_replaces_the_old() {
        let plan = vec![a("154", "4", 3, AssignState::Asked), a("154", "1", 1, AssignState::Taken)];
        let msgs = encode_plan(7, &plan);
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().all(|m| m.chars().count() < omsi_net::MAX_CHAT));
        let mut d = Dispatch::default();
        for m in &msgs {
            apply_plan(&mut d, m.strip_prefix("plan ").unwrap());
        }
        assert_eq!(d.gen, 7);
        assert_eq!(d.plan, plan);
        // an older plan is ignored, a newer empty one clears it
        assert!(!apply_plan(&mut d, "6¦0"));
        assert_eq!(d.plan.len(), 2);
        let empty = encode_plan(8, &[]);
        assert!(apply_plan(&mut d, empty[0].strip_prefix("plan ").unwrap()));
        assert!(d.plan.is_empty());
        assert!(!apply_plan(&mut d, "nonsense"));
    }

    #[test]
    fn a_player_has_one_duty_and_only_the_one_asked_answers() {
        let mut plan = Vec::new();
        give(&mut plan, "154", "4", 3, "Kev");
        give(&mut plan, "154", "5", 3, "Kev");
        assert_eq!(plan.len(), 1, "his first duty is free again");
        assert_eq!((plan[0].tour.as_str(), plan[0].state), ("5", AssignState::Asked));
        give(&mut plan, "154", "5", 4, "Lena");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].pid, 4);
        assert!(!answer(&mut plan, "154", "5", 3, true), "Kev was not asked any more");
        assert!(answer(&mut plan, "154", "5", 4, true));
        assert_eq!(plan[0].state, AssignState::Taken);
        assert!(free(&mut plan, "154", "5"));
        assert!(!free(&mut plan, "154", "5"));
    }

    #[test]
    fn an_ask_goes_over_and_nonsense_is_refused() {
        let r = Request { line: "154".into(), tour: "4".into(), start: 8.0 * 3600.0 + 40.0 * 60.0, end: 17.0 * 3600.0 + 600.0, by: "Tobi_96".into() };
        let text = encode_ask(&r);
        assert!(text.chars().count() < omsi_net::MAX_CHAT);
        assert_eq!(parse_ask(text.strip_prefix("ask ").unwrap()), Some(r));
        assert!(parse_ask("154¦4¦NaN¦0¦x").is_none());
        assert!(parse_ask("¦4¦0¦0¦x").is_none());
        assert!(parse_ask("154").is_none());
    }

    #[test]
    fn the_company_lines_come_first_and_the_list_scrolls_in_range() {
        let all: Vec<String> = ["100", "154", "197"].iter().map(|s| s.to_string()).collect();
        assert_eq!(board_lines(&["197".into(), "999".into()], &all), vec!["197".to_string()]);
        assert_eq!(board_lines(&[], &all), all);
        assert_eq!(clamp_scroll(10, 12, 8), 4);
        assert_eq!(clamp_scroll(3, 5, 8), 0);
        assert_eq!(duty_name("154", "4"), "154-4");
        assert_eq!(board_line(&all, Some("154")), Some("154".to_string()));
        assert_eq!(board_line(&all, Some("999")), Some("100".to_string()));
        assert_eq!(board_line(&[], None), None);
    }
}
