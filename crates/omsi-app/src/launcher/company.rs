//! The launcher's Company page: found a bus company (`omsi_launcher_lib::company`), then
//! its overview, fleet, lines, staff and finances. The company moves on when the player
//! drives: opening the page books the runs the game wrote since (their takings and costs,
//! and the company time in which the hired drivers work).

use super::theme::*;
use super::ui::{id_of, ButtonKind, Ui};
use super::{Launcher, Page};
use glam::Vec2;
use omsi_launcher_lib as core;
use core::company::{self, money, Company, Hire, Level, COLORS};
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};

/// The page's state between frames.
pub struct CompanyView {
    pub company: Option<Company>,
    pub loaded: bool,
    pub tab: usize,
    // the founding form
    pub name: String,
    pub short: String,
    pub color: usize,
    pub map: usize,
    pub depot: String,
    pub level: usize,
    pub share: bool,
    /// Drivers applying now, and the number that changes them.
    pub candidates: Vec<Hire>,
    seed: u64,
    pub shop_filter: String,
    /// A dangerous button clicked once (its name) and when: a second click does it.
    armed: Option<(String, std::time::Instant)>,
}

impl Default for CompanyView {
    fn default() -> Self {
        CompanyView {
            company: None,
            loaded: false,
            tab: 0,
            name: String::new(),
            short: String::new(),
            color: 0,
            map: 0,
            depot: "Depot".into(),
            level: 1,
            share: false,
            candidates: Vec::new(),
            seed: 1,
            shop_filter: String::new(),
            armed: None,
        }
    }
}

/// The tabs.
const TABS: [&str; 5] = ["Overview", "Fleet", "Lines", "Staff", "Finances"];

fn tr(s: &str) -> String {
    omsi_ui::tr(s).into_owned()
}

/// A booking's text in the interface's language ("Line 136 · 24.0 km", "Free drive · …").
fn entry_text(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("Line ") {
        format!("{} {rest}", tr("Line"))
    } else if let Some(rest) = s.strip_prefix("Free drive") {
        format!("{}{rest}", tr("Free drive"))
    } else {
        s.to_string()
    }
}

fn rgb(c: u32) -> Color {
    Color::rgba((c >> 16) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8, 1.0)
}

fn condition_color(c: f64) -> Color {
    if c < company::WORKSHOP_BELOW {
        DANGER
    } else if c < 70.0 {
        WARN
    } else {
        OK
    }
}

impl CompanyView {
    /// Read the company and book the runs the game wrote since the last look.
    pub fn refresh(&mut self, l: &mut super::state::State) {
        self.loaded = true;
        // the prices and wear the player may have changed (written with the defaults the first time)
        if let Err(e) = company::load_tuning(&core::company_values_path()) {
            log::warn!("company values: {e}");
        }
        self.company = company::Company::load(&core::company_path());
        if self.candidates.is_empty() {
            self.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1);
            self.candidates = company::candidates(self.seed);
        }
        let Some(c) = self.company.as_mut() else { return };
        let mut n = 0;
        let mut result = 0.0;
        for run in core::company_runs() {
            let before = c.balance;
            if c.book(&run).is_some() {
                n += 1;
                result += c.balance - before;
            }
        }
        if n > 0 {
            let _ = c.save(&core::company_path());
            l.set_status(format!("{}: {n} · {}", tr("Runs booked for your company"), money(result)), false);
        }
    }

    fn save(&mut self, state: &mut super::state::State) {
        if let Some(c) = &self.company {
            if let Err(e) = c.save(&core::company_path()) {
                state.set_status(format!("{}: {e}", tr("The company could not be saved")), true);
            }
        }
    }

    /// A dangerous button: the first click arms it for four seconds, the second is true.
    fn armed(&mut self, name: &str) -> bool {
        self.armed.as_ref().map(|(n, t)| n == name && t.elapsed().as_secs() < 4).unwrap_or(false)
    }

    fn arm(&mut self, name: &str) {
        self.armed = Some((name.to_string(), std::time::Instant::now()));
    }
}

/// What a click inside a list asks for (done after the list is drawn).
enum Act {
    Buy(String, String),
    Sell(u32),
    Repair(u32),
    AssignBus(u32, String),
    Take(String),
    Drop(String),
    Hire(usize),
    Fire(usize),
    AssignDriver(usize, String),
}

pub fn draw(l: &mut Launcher, area: Rect) {
    if !l.company.loaded {
        l.company.refresh(&mut l.state);
    }
    // the lines of the company's map (the Drive page's map becomes the company's)
    if let Some(c) = &l.company.company {
        if l.state.choice.map != c.map && l.state.maps.iter().any(|m| m.file == c.map) {
            l.state.choice.map = c.map.clone();
            l.state.load_lines();
        }
    }
    if l.company.company.is_none() {
        found(l, area);
        return;
    }
    let (name, map) = {
        let c = l.company.company.as_ref().unwrap();
        (c.name.clone(), c.map.clone())
    };
    let map_name = l.state.maps.iter().find(|m| m.file == map).map(|m| if m.friendly.is_empty() { m.name.clone() } else { m.friendly.clone() }).unwrap_or(map.clone());
    let sub = format!("{name} · {} {map_name}", tr("home map"));
    let body = l.page_title(area, "Company", &sub);
    // the tabs, top right
    let tw = 520.0f32.min(body.w);
    let mut tab = l.company.tab;
    if l.ui.segmented("company-tabs", Rect::new(body.right() - tw, area.y + 4.0, tw, 34.0), &mut tab, &TABS) {
        l.company.tab = tab;
    }
    match l.company.tab {
        0 => overview(l, body),
        1 => fleet(l, body),
        2 => lines(l, body),
        3 => staff(l, body),
        _ => finances(l, body),
    }
}

// --- founding -------------------------------------------------------------------------------

fn found(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, "Found a company", "Your own bus company: buy buses, take over lines, hire drivers - and earn the money with every duty you drive.");
    let left_w = (body.w * 0.6).min(760.0);
    let left = Rect::new(body.x, body.y, left_w, body.h.min(600.0));
    let right = Rect::new(left.right() + GAP * 2.0, body.y, body.w - left_w - GAP * 2.0, 330.0);
    l.ui.panel(left);
    let inner = left.pad(22.0, 18.0);
    let half = (inner.w - GAP) * 0.5;
    let mut y = inner.y;
    l.ui.heading(Rect::new(inner.x, y, inner.w, 26.0), "1 · Your company", None);
    y += 30.0;
    l.ui.label(Rect::new(inner.x, y, half, 18.0), "Name");
    l.ui.label(Rect::new(inner.x + half + GAP, y, half, 18.0), "Short name (on buses and tickets)");
    y += 22.0;
    l.ui.text_input("co-name", Rect::new(inner.x, y, half, ROW), &mut l.company.name, "Spandauer Verkehrsbetriebe", None);
    l.ui.text_input("co-short", Rect::new(inner.x + half + GAP, y, half, ROW), &mut l.company.short, "SVB", None);
    y += ROW + 14.0;
    l.ui.label(Rect::new(inner.x, y, half, 18.0), "Home map");
    l.ui.label(Rect::new(inner.x + half + GAP, y, half, 18.0), "Depot");
    y += 22.0;
    let maps: Vec<String> = l.state.maps.iter().map(|m| if m.friendly.is_empty() { m.name.clone() } else { m.friendly.clone() }).collect();
    if l.company.map >= maps.len() {
        l.company.map = l.state.maps.iter().position(|m| m.file == l.state.choice.map).unwrap_or(0);
    }
    if !maps.is_empty() {
        let mut m = l.company.map;
        if l.ui.select("co-map", Rect::new(inner.x, y, half, ROW), &mut m, &maps) {
            l.company.map = m;
        }
    } else {
        l.ui.text_in("No maps found (Setup)", Rect::new(inner.x, y, half, ROW), 13.0, Weight::Regular, DANGER, Align::Left);
    }
    l.ui.text_input("co-depot", Rect::new(inner.x + half + GAP, y, half, ROW), &mut l.company.depot, "Depot", None);
    y += ROW + 14.0;
    l.ui.label(Rect::new(inner.x, y, inner.w, 18.0), "Company colour");
    y += 24.0;
    for (k, (cname, c)) in COLORS.iter().enumerate() {
        let cc = Vec2::new(inner.x + 18.0 + k as f32 * 46.0, y + 16.0);
        let r = Rect::new(cc.x - 18.0, cc.y - 18.0, 36.0, 36.0);
        let (h, _, clicked) = l.ui.interact(id_of(&format!("co-color-{k}")), r);
        if clicked {
            l.company.color = k;
        }
        if l.company.color == k {
            l.ui.p().circle(cc, 18.0, TEXT);
        }
        l.ui.p().circle(cc, if l.company.color == k { 15.0 } else { 16.0 }, rgb(*c));
        if h {
            l.ui.tooltip(r, cname);
        }
    }
    y += 46.0;
    l.ui.heading(Rect::new(inner.x, y, inner.w, 26.0), "2 · Difficulty", None);
    y += 30.0;
    let cw = (inner.w - GAP * 2.0) / 3.0;
    let texts = [
        "Much money to start, cheap diesel, passengers forgive a late bus.",
        "Balanced: punctuality and the state of the buses count.",
        "Little money, dear diesel, high fines.",
    ];
    for (k, lv) in Level::ALL.iter().enumerate() {
        let r = Rect::new(inner.x + k as f32 * (cw + GAP), y, cw, 112.0);
        let (h, _, clicked) = l.ui.interact(id_of(&format!("co-level-{k}")), r);
        if clicked {
            l.company.level = k;
        }
        let sel = l.company.level == k;
        l.ui.p().rounded(r, 10.0, if h { HOVER } else { FIELD });
        l.ui.p().rounded_border(r, 10.0, if sel { 2.0 } else { 1.0 }, if sel { ACCENT } else { EDGE });
        l.ui.text_in(lv.name(), Rect::new(r.x + 14.0, r.y + 10.0, r.w - 28.0, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
        l.ui.text_in(&money(lv.start_money()), Rect::new(r.x + 14.0, r.y + 32.0, r.w - 28.0, 24.0), 18.0, Weight::Black, ACCENT, Align::Left);
        l.ui.paragraph(texts[k], Vec2::new(r.x + 14.0, r.y + 64.0), r.w - 28.0, 11.5, Weight::Regular, TEXT_DIM);
    }
    y += 112.0 + 20.0;
    l.ui.heading(Rect::new(inner.x, y, inner.w, 26.0), "3 · Multiplayer", None);
    y += 30.0;
    let mut share = l.company.share;
    l.ui.toggle("co-share", Rect::new(inner.x, y, inner.w, 28.0), &mut share, "Share the company with friends in multiplayer");
    l.company.share = share;

    // preview and the button
    l.ui.panel(right);
    let inner = right.pad(22.0, 18.0);
    l.ui.heading(Rect::new(inner.x, inner.y, inner.w, 26.0), "Preview", None);
    let name = if l.company.name.trim().is_empty() { tr("Your company") } else { l.company.name.trim().to_string() };
    let short = if l.company.short.trim().is_empty() { name.chars().filter(|c| c.is_uppercase()).take(3).collect::<String>() } else { l.company.short.trim().to_string() };
    let logo = Rect::new(inner.x, inner.y + 34.0, 56.0, 56.0);
    l.ui.p().rounded(logo, 12.0, rgb(COLORS[l.company.color].1));
    l.ui.text_in(&short, logo, 16.0, Weight::Black, Color::rgba(19, 19, 19, 1.0), Align::Center);
    l.ui.text_in(&name, Rect::new(logo.right() + 14.0, logo.y + 6.0, inner.w - 70.0, 22.0), 16.0, Weight::Bold, TEXT, Align::Left);
    let map_name = maps.get(l.company.map).cloned().unwrap_or_default();
    l.ui.text_in(&format!("{map_name} · {}", l.company.depot), Rect::new(logo.right() + 14.0, logo.y + 30.0, inner.w - 70.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    let lv = Level::from_index(l.company.level);
    let row = |ui: &mut Ui, y: f32, a: &str, b: &str| {
        ui.text_in(a, Rect::new(inner.x, y, inner.w * 0.6, 20.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        ui.text_in(b, Rect::new(inner.x + inner.w * 0.4, y, inner.w * 0.6, 20.0), 13.0, Weight::Bold, TEXT, Align::Right);
    };
    row(&mut l.ui, inner.y + 110.0, "Starting capital", &money(lv.start_money()));
    row(&mut l.ui, inner.y + 136.0, "Difficulty", &tr(lv.name()));
    row(&mut l.ui, inner.y + 162.0, "Free lines on the map", &tr("all"));
    let ok = !l.company.name.trim().is_empty() && !l.state.maps.is_empty();
    if l.ui.button("co-found", Rect::new(inner.x, inner.y + 206.0, inner.w, 44.0), "Found the company", Some("check_circle"), if ok { ButtonKind::Primary } else { ButtonKind::Normal }) {
        if !ok {
            l.state.set_status(tr("Give the company a name first"), true);
        } else if let Some(m) = l.state.maps.get(l.company.map) {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let mut c = Company::found(&name, &short, l.company.color, &m.file, l.company.depot.trim(), lv, now);
            c.shared = l.company.share;
            l.company.company = Some(c);
            l.company.tab = 1;
            l.company.save(&mut l.state);
            l.state.set_status(format!("{}: {name}", tr("Company founded")), false);
        }
    }
}

// --- overview -------------------------------------------------------------------------------

fn tile(ui: &mut Ui, r: Rect, label: &str, value: &str, sub: &str, sub_c: Color) {
    ui.panel(r);
    ui.text_in(&tr(label).to_uppercase(), Rect::new(r.x + 16.0, r.y + 10.0, r.w - 32.0, 18.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    ui.text_in(value, Rect::new(r.x + 16.0, r.y + 32.0, r.w - 32.0, 32.0), 24.0, Weight::Black, TEXT, Align::Left);
    ui.text_in(sub, Rect::new(r.x + 16.0, r.y + 66.0, r.w - 32.0, 18.0), 11.5, Weight::Medium, sub_c, Align::Left);
}

fn overview(l: &mut Launcher, body: Rect) {
    let c = l.company.company.clone().unwrap();
    let day = c.day();
    let (inc, exp) = c.sums(day.saturating_sub(6).max(1), day + 1);
    let cols = 4.0;
    let tw = (body.w - GAP * (cols - 1.0)) / cols;
    let th = 92.0;
    let t = |k: f32| Rect::new(body.x + k * (tw + GAP), body.y, tw, th);
    tile(&mut l.ui, t(0.0), "Balance", &money(c.balance), &format!("{} {}", tr("Day"), day), if c.balance < 0.0 { DANGER } else { TEXT_DIM });
    tile(&mut l.ui, t(1.0), "Income (7 days)", &money(inc), &tr("Tickets, bonus, subsidy"), TEXT_DIM);
    tile(&mut l.ui, t(2.0), "Costs (7 days)", &money(exp), &tr("Diesel, wages, workshop"), TEXT_DIM);
    tile(&mut l.ui, t(3.0), "Punctuality", &format!("{:.0} %", c.punctuality() * 100.0), &format!("{} {}", c.stops, tr("stops served")), TEXT_DIM);
    let y = body.y + th + GAP;
    let h = (body.h - th - GAP).max(200.0);
    let lw = (body.w * 0.58).floor();
    // income and costs by week
    let chart = Rect::new(body.x, y, lw, h * 0.55);
    l.ui.panel(chart);
    let inner = l.ui.heading(chart.pad(18.0, 12.0), "Income and costs (last 8 weeks)", None);
    let weeks = c.weeks(8);
    let top = weeks.iter().map(|w| w.0.max(w.1)).fold(1.0, f64::max);
    let plot = Rect::new(inner.x, inner.y + 8.0, inner.w, inner.h - 30.0);
    let slot = plot.w / weeks.len() as f32;
    for (k, (i, e)) in weeks.iter().enumerate() {
        let x = plot.x + k as f32 * slot + slot * 0.5;
        let hi = (*i / top) as f32 * plot.h;
        let he = (*e / top) as f32 * plot.h;
        l.ui.p().rounded(Rect::new(x - 13.0, plot.bottom() - hi, 12.0, hi.max(1.0)), 3.0, ACCENT);
        l.ui.p().rounded(Rect::new(x + 1.0, plot.bottom() - he, 12.0, he.max(1.0)), 3.0, Color::rgba(90, 90, 90, 1.0));
        let wk = (day as i64 - 1) / 7 - (weeks.len() as i64 - 1 - k as i64) + 1;
        let label = if wk >= 1 { format!("{} {wk}", tr("Wk")) } else { String::new() };
        l.ui.text_in(&label, Rect::new(x - slot * 0.5, plot.bottom() + 6.0, slot, 16.0), 10.5, Weight::Regular, TEXT_FAINT, Align::Center);
    }
    // the player's last trips
    let trips = Rect::new(body.x, chart.bottom() + GAP, lw, h - chart.h - GAP);
    l.ui.panel(trips);
    let inner = l.ui.heading(trips.pad(18.0, 12.0), "Your last trips", None);
    if c.trips.is_empty() {
        l.ui.text_in("No trips yet. Drive a duty on the company's map and it is booked here.", Rect::new(inner.x, inner.y, inner.w, 24.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    let rh = 30.0;
    for (k, tp) in c.trips.iter().enumerate() {
        let r = Rect::new(inner.x, inner.y + k as f32 * rh, inner.w, rh);
        if r.bottom() > trips.bottom() - 8.0 {
            break;
        }
        let line = if tp.line.is_empty() { tr("Free drive") } else { format!("{} {}", tr("Line"), tp.line) };
        l.ui.text_in(&line, Rect::new(r.x, r.y, 120.0, rh), 13.0, Weight::Bold, TEXT, Align::Left);
        l.ui.text_in(&format!("{} · {:.1} km · {:.0} % {}", tp.bus, tp.km, tp.punctual * 100.0, tr("on time")), Rect::new(r.x + 120.0, r.y, r.w - 240.0, rh), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        l.ui.text_in(&money(tp.result), Rect::new(r.right() - 120.0, r.y, 120.0, rh), 13.0, Weight::Bold, if tp.result >= 0.0 { OK } else { DANGER }, Align::Right);
    }
    // what to do
    let side = Rect::new(body.x + lw + GAP, y, body.w - lw - GAP, h);
    l.ui.panel(side);
    let inner = l.ui.heading(side.pad(18.0, 12.0), "Notices", None);
    let free = l.state.lines.iter().filter(|x| !c.lines.contains(&x.name)).count();
    let notes = c.notices(free);
    let mut yy = inner.y;
    if notes.is_empty() {
        l.ui.text_in("All is well.", Rect::new(inner.x, yy, inner.w, 22.0), 13.0, Weight::Regular, OK, Align::Left);
        yy += 30.0;
    }
    for (sev, tmpl, val) in notes.iter().take(8) {
        let col = [ACCENT_2, WARN, DANGER][(*sev as usize).min(2)];
        l.ui.p().circle(Vec2::new(inner.x + 5.0, yy + 11.0), 4.0, col);
        let text = tr(tmpl).replace("{}", val);
        let hgt = l.ui.paragraph(&text, Vec2::new(inner.x + 18.0, yy + 4.0), inner.w - 18.0, 12.5, Weight::Regular, TEXT_SOFT);
        yy += hgt.max(18.0) + 12.0;
    }
    // drive for the company: the Drive page with the company's map and a bus of its own
    let br = Rect::new(inner.x, side.bottom() - 18.0 - 44.0, inner.w, 44.0);
    if l.ui.button("co-drive", br, "Drive a duty for the company", Some("directions_bus"), ButtonKind::Primary) {
        l.state.choice.map = c.map.clone();
        if let Some(b) = c.buses.iter().filter(|b| b.condition > 0.0).max_by(|a, b| a.condition.total_cmp(&b.condition)) {
            l.state.choice.bus = b.file.clone();
        }
        l.state.load_lines();
        l.go(Page::Drive);
    }
}

// --- fleet ----------------------------------------------------------------------------------

fn fleet(l: &mut Launcher, body: Rect) {
    let c = l.company.company.clone().unwrap();
    let lw = (body.w * 0.62).floor();
    let left = Rect::new(body.x, body.y, lw, body.h);
    let right = Rect::new(body.x + lw + GAP, body.y, body.w - lw - GAP, body.h);
    let mut acts: Vec<Act> = Vec::new();
    // the company's buses
    l.ui.panel(left);
    let inner = l.ui.heading(left.pad(18.0, 12.0), &format!("{} ({})", tr("Your buses"), c.buses.len()), None);
    let mut line_opts: Vec<String> = vec![tr("Depot")];
    line_opts.extend(c.lines.iter().map(|x| format!("{} {x}", tr("Line"))));
    let armed_sell = l.company.armed.clone();
    let cols = [0.0f32, 0.09, 0.36, 0.56, 0.66, 0.83];
    let head = ["No.", "Bus", "Condition", "km", "Line", ""];
    for (k, h) in head.iter().enumerate() {
        l.ui.text_in(h, Rect::new(inner.x + inner.w * cols[k], inner.y, 120.0, 18.0), 11.0, Weight::Bold, TEXT_FAINT, Align::Left);
    }
    let list = Rect::new(inner.x - 6.0, inner.y + 24.0, inner.w + 12.0, inner.h - 24.0);
    l.ui.scroll_area("co-fleet", list, &mut |ui, v| {
        if c.buses.is_empty() {
            ui.text_in("No buses yet: buy your first one on the right.", Rect::new(v.x + 6.0, v.y + 4.0, v.w, 24.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
            return 30.0;
        }
        let rh = 52.0;
        let w = v.w - 12.0;
        for (k, b) in c.buses.iter().enumerate() {
            let y = v.y + k as f32 * rh;
            let x = v.x + 6.0;
            ui.p().rounded(Rect::new(x, y + 2.0, w, rh - 6.0), 8.0, Color::WHITE.alpha(0.03));
            let cy = y + 2.0 + (rh - 6.0) * 0.5 - 10.0;
            ui.text_in(&b.nr.to_string(), Rect::new(x + 8.0, cy, w * 0.09, 20.0), 13.0, Weight::Bold, TEXT, Align::Left);
            ui.text_in(&b.name, Rect::new(x + w * cols[1], cy, w * 0.26, 20.0), 13.0, Weight::Medium, TEXT, Align::Left);
            ui.progress(Rect::new(x + w * cols[2], cy + 6.0, w * 0.13, 8.0), (b.condition / 100.0) as f32, false);
            ui.text_in(&format!("{:.0} %", b.condition), Rect::new(x + w * cols[2] + w * 0.14, cy, w * 0.06, 20.0), 11.5, Weight::Bold, condition_color(b.condition), Align::Left);
            ui.text_in(&format!("{:.0}", b.km), Rect::new(x + w * cols[3], cy, w * 0.09, 20.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            let mut sel = c.lines.iter().position(|x| *x == b.line).map(|i| i + 1).unwrap_or(0);
            if ui.select(&format!("co-bus-line-{}", b.nr), Rect::new(x + w * cols[4], y + 9.0, w * 0.16, 30.0), &mut sel, &line_opts) {
                acts.push(Act::AssignBus(b.nr, if sel == 0 { String::new() } else { c.lines[sel - 1].clone() }));
            }
            let bw = (w * 0.17 - 6.0) * 0.5;
            let bx = x + w * cols[5];
            if b.condition < 99.5 && ui.button(&format!("co-repair-{}", b.nr), Rect::new(bx, y + 9.0, bw, 30.0), "Workshop", None, ButtonKind::Normal) {
                acts.push(Act::Repair(b.nr));
            }
            let key = format!("co-sell-{}", b.nr);
            let armed = armed_sell.as_ref().map(|(n, t)| *n == key && t.elapsed().as_secs() < 4).unwrap_or(false);
            if ui.button(&key, Rect::new(bx + bw + 6.0, y + 9.0, bw, 30.0), if armed { "Sure?" } else { "Sell" }, None, ButtonKind::Danger) {
                acts.push(Act::Sell(b.nr));
            }
        }
        c.buses.len() as f32 * rh
    });
    // the shop: every vehicle of the installation
    l.ui.panel(right);
    let inner = l.ui.heading(right.pad(18.0, 12.0), "Buy a bus", None);
    l.ui.text_input("co-shop-filter", Rect::new(inner.x, inner.y, inner.w, ROW), &mut l.company.shop_filter, "Search buses", Some("search"));
    let filter = l.company.shop_filter.to_lowercase();
    let vehicles: Vec<(String, String, bool)> = l
        .state
        .vehicles
        .iter()
        .filter(|v| filter.is_empty() || v.name.to_lowercase().contains(&filter) || v.manufacturer.to_lowercase().contains(&filter))
        .map(|v| {
            let n = core::display_bus_name(&v.name);
            (v.file.clone(), if v.manufacturer.is_empty() || n.starts_with(&v.manufacturer) { n } else { format!("{} {n}", v.manufacturer) }, v.installed)
        })
        .collect();
    let balance = c.balance;
    let list = Rect::new(inner.x - 6.0, inner.y + ROW + 10.0, inner.w + 12.0, inner.h - ROW - 10.0);
    l.ui.scroll_area("co-shop", list, &mut |ui, v| {
        if vehicles.is_empty() {
            ui.text_in("No buses found.", Rect::new(v.x + 6.0, v.y + 4.0, v.w, 24.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
            return 30.0;
        }
        let rh = 58.0;
        for (k, (file, name, is_mod)) in vehicles.iter().enumerate() {
            let r = Rect::new(v.x + 6.0, v.y + k as f32 * rh, v.w - 12.0, rh - 6.0);
            if !ui.rect_visible(r) {
                continue;
            }
            ui.p().rounded(r, 8.0, Color::WHITE.alpha(0.03));
            ui.icon("directions_bus", Vec2::new(r.x + 22.0, r.center().y), 20.0, TEXT_DIM);
            ui.text_in(name, Rect::new(r.x + 42.0, r.y + 6.0, r.w - 170.0, 20.0), 13.0, Weight::Medium, TEXT, Align::Left);
            let price = company::price_for(name, file);
            ui.text_in(&format!("{}{}", money(price), if *is_mod { format!(" · {}", tr("mod")) } else { String::new() }), Rect::new(r.x + 42.0, r.y + 26.0, r.w - 170.0, 18.0), 11.5, Weight::Regular, if price > balance { DANGER } else { TEXT_DIM }, Align::Left);
            if ui.button(&format!("co-buy-{k}"), Rect::new(r.right() - 100.0, r.y + 10.0, 90.0, 32.0), "Buy", Some("add"), ButtonKind::Normal) {
                acts.push(Act::Buy(file.clone(), name.clone()));
            }
        }
        vehicles.len() as f32 * rh
    });
    apply(l, acts);
}

// --- lines ----------------------------------------------------------------------------------

fn lines(l: &mut Launcher, body: Rect) {
    let c = l.company.company.clone().unwrap();
    let mut acts: Vec<Act> = Vec::new();
    l.ui.panel(body);
    let inner = l.ui.heading(body.pad(18.0, 12.0), "Lines of the home map", None);
    if l.state.loading_lines {
        l.ui.text_in("Reading the timetable…", Rect::new(inner.x, inner.y, inner.w, 24.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    }
    let all: Vec<(String, String, usize)> = l.state.lines.iter().filter(|x| x.user_allowed || !x.tours.is_empty()).map(|x| (x.name.clone(), x.termini.join(" – "), x.tours.len())).collect();
    let cols = [0.0f32, 0.1, 0.52, 0.62, 0.72, 0.82];
    let head = ["Line", "Route", "Tours", "Buses", "Drivers", ""];
    for (k, h) in head.iter().enumerate() {
        l.ui.text_in(h, Rect::new(inner.x + inner.w * cols[k], inner.y, 120.0, 18.0), 11.0, Weight::Bold, TEXT_FAINT, Align::Left);
    }
    let armed = l.company.armed.clone();
    let list = Rect::new(inner.x - 6.0, inner.y + 24.0, inner.w + 12.0, inner.h - 24.0);
    l.ui.scroll_area("co-lines", list, &mut |ui, v| {
        if all.is_empty() {
            ui.text_in("This map has no timetable lines.", Rect::new(v.x + 6.0, v.y + 4.0, v.w, 24.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
            return 30.0;
        }
        let rh = 50.0;
        let w = v.w - 12.0;
        for (k, (name, route, tours)) in all.iter().enumerate() {
            let y = v.y + k as f32 * rh;
            let x = v.x + 6.0;
            let r = Rect::new(x, y + 2.0, w, rh - 6.0);
            if !ui.rect_visible(r) {
                continue;
            }
            let mine = c.lines.contains(name);
            ui.p().rounded(r, 8.0, Color::WHITE.alpha(0.03));
            let badge = Rect::new(x + 8.0, y + 12.0, (w * 0.08).max(44.0), 24.0);
            ui.p().rounded(badge, 6.0, if mine { ACCENT } else { SELECTED });
            ui.text_in(name, badge, 13.0, Weight::Bold, if mine { Color::rgba(18, 14, 8, 1.0) } else { TEXT }, Align::Center);
            ui.text_in(route, Rect::new(x + w * cols[1], y + 2.0, w * 0.4, rh - 6.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
            ui.text_in(&tours.to_string(), Rect::new(x + w * cols[2], y + 2.0, w * 0.08, rh - 6.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
            let nb = c.buses.iter().filter(|b| &b.line == name).count();
            let nd = c.drivers.iter().filter(|d| &d.line == name).count();
            ui.text_in(&nb.to_string(), Rect::new(x + w * cols[3], y + 2.0, w * 0.08, rh - 6.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(&nd.to_string(), Rect::new(x + w * cols[4], y + 2.0, w * 0.08, rh - 6.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
            let br = Rect::new(x + w * cols[5], y + 9.0, w * 0.17, 30.0);
            if mine {
                let key = format!("co-drop-{name}");
                let sure = armed.as_ref().map(|(n, t)| *n == key && t.elapsed().as_secs() < 4).unwrap_or(false);
                if ui.button(&key, br, if sure { "Sure?" } else { "Give back" }, None, ButtonKind::Ghost) {
                    acts.push(Act::Drop(name.clone()));
                }
            } else if ui.button(&format!("co-take-{name}"), br, "Take over", Some("add"), ButtonKind::Primary) {
                acts.push(Act::Take(name.clone()));
            }
        }
        all.len() as f32 * rh
    });
    apply(l, acts);
}

// --- staff ----------------------------------------------------------------------------------

fn staff(l: &mut Launcher, body: Rect) {
    let c = l.company.company.clone().unwrap();
    let mut acts: Vec<Act> = Vec::new();
    let lw = (body.w * 0.62).floor();
    let left = Rect::new(body.x, body.y, lw, body.h);
    let right = Rect::new(body.x + lw + GAP, body.y, body.w - lw - GAP, body.h);
    l.ui.panel(left);
    let inner = l.ui.heading(left.pad(18.0, 12.0), &format!("{} ({})", tr("Your drivers"), c.drivers.len()), None);
    let mut line_opts: Vec<String> = vec![tr("No line")];
    line_opts.extend(c.lines.iter().map(|x| format!("{} {x}", tr("Line"))));
    let armed = l.company.armed.clone();
    let list = Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h);
    l.ui.scroll_area("co-staff", list, &mut |ui, v| {
        let rh = 56.0;
        let w = v.w - 12.0;
        // the player first: the boss drives too
        let x = v.x + 6.0;
        ui.p().rounded(Rect::new(x, v.y + 2.0, w, rh - 6.0), 8.0, Color::WHITE.alpha(0.03));
        ui.icon("account_circle", Vec2::new(x + 24.0, v.y + rh * 0.5 - 1.0), 26.0, ACCENT);
        ui.text_in("You", Rect::new(x + 46.0, v.y + 8.0, w * 0.4, 20.0), 13.0, Weight::Bold, TEXT, Align::Left);
        ui.text_in("Boss · drives any line", Rect::new(x + 46.0, v.y + 28.0, w * 0.4, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
        if c.drivers.is_empty() {
            ui.text_in("No drivers hired yet: hire one on the right and give them a line with a bus.", Rect::new(x, v.y + rh + 6.0, w, 22.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
            return rh + 34.0;
        }
        for (k, d) in c.drivers.iter().enumerate() {
            let y = v.y + (k + 1) as f32 * rh;
            let r = Rect::new(x, y + 2.0, w, rh - 6.0);
            if !ui.rect_visible(r) {
                continue;
            }
            ui.p().rounded(r, 8.0, Color::WHITE.alpha(0.03));
            ui.icon("person", Vec2::new(x + 24.0, y + rh * 0.5 - 1.0), 22.0, TEXT_DIM);
            ui.text_in(&d.name, Rect::new(x + 46.0, y + 8.0, w * 0.3, 20.0), 13.0, Weight::Medium, TEXT, Align::Left);
            ui.text_in(&format!("{} {}", money(d.wage), tr("a month")), Rect::new(x + 46.0, y + 28.0, w * 0.3, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in("Skill", Rect::new(x + w * 0.36, y + 8.0, w * 0.16, 16.0), 10.5, Weight::Bold, TEXT_FAINT, Align::Left);
            ui.progress(Rect::new(x + w * 0.36, y + 30.0, w * 0.16, 8.0), d.skill as f32, false);
            let mut sel = c.lines.iter().position(|x| *x == d.line).map(|i| i + 1).unwrap_or(0);
            if ui.select(&format!("co-drv-line-{k}"), Rect::new(x + w * 0.56, y + 11.0, w * 0.22, 30.0), &mut sel, &line_opts) {
                acts.push(Act::AssignDriver(k, if sel == 0 { String::new() } else { c.lines[sel - 1].clone() }));
            }
            let key = format!("co-fire-{k}");
            let sure = armed.as_ref().map(|(n, t)| *n == key && t.elapsed().as_secs() < 4).unwrap_or(false);
            if ui.button(&key, Rect::new(x + w * 0.81, y + 11.0, w * 0.17, 30.0), if sure { "Sure?" } else { "Dismiss" }, None, ButtonKind::Danger) {
                acts.push(Act::Fire(k));
            }
        }
        (c.drivers.len() + 1) as f32 * rh
    });
    // applicants, and the friends (next stage)
    l.ui.panel(right);
    let inner = l.ui.heading(right.pad(18.0, 12.0), "Applicants", None);
    let cands = l.company.candidates.clone();
    let tiers = ["Beginner", "Experienced", "Professional"];
    let mut y = inner.y;
    for (k, h) in cands.iter().enumerate() {
        let r = Rect::new(inner.x, y, inner.w, 64.0);
        l.ui.p().rounded(r, 8.0, Color::WHITE.alpha(0.03));
        l.ui.text_in(&h.name, Rect::new(r.x + 12.0, r.y + 8.0, r.w - 130.0, 20.0), 13.0, Weight::Medium, TEXT, Align::Left);
        l.ui.text_in(&format!("{} · {} {}", tr(tiers[k.min(2)]), money(h.wage), tr("a month")), Rect::new(r.x + 12.0, r.y + 30.0, r.w - 130.0, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
        l.ui.progress(Rect::new(r.x + 12.0, r.y + 52.0, r.w - 140.0, 5.0), h.skill as f32, false);
        if l.ui.button(&format!("co-hire-{k}"), Rect::new(r.right() - 108.0, r.y + 16.0, 96.0, 32.0), "Hire", Some("add"), ButtonKind::Normal) {
            acts.push(Act::Hire(k));
        }
        y += 72.0;
    }
    if l.ui.button("co-new-applicants", Rect::new(inner.x, y + 4.0, inner.w, ROW), "Other applicants", Some("refresh"), ButtonKind::Ghost) {
        l.company.seed = l.company.seed.wrapping_add(1);
        l.company.candidates = company::candidates(l.company.seed);
    }
    y += ROW + 24.0;
    l.ui.heading(Rect::new(inner.x, y, inner.w, 26.0), "Friends", None);
    let mut share = c.shared;
    if l.ui.toggle("co-share-staff", Rect::new(inner.x, y + 30.0, inner.w, 28.0), &mut share, "Friends drive for the company") {
        if let Some(co) = l.company.company.as_mut() {
            co.shared = share;
        }
        l.company.save(&mut l.state);
    }
    l.ui.paragraph("Host a multiplayer session on the company's map: the friends who join see its account, and what they earn is booked here after the session.", Vec2::new(inner.x, y + 66.0), inner.w, 12.5, Weight::Regular, TEXT_DIM);
    apply(l, acts);
}

// --- finances -------------------------------------------------------------------------------

fn finances(l: &mut Launcher, body: Rect) {
    let c = l.company.company.clone().unwrap();
    let day = c.day();
    let from = day.saturating_sub(29).max(1);
    let kinds = c.by_kind(from, day + 1);
    let cw = (body.w - GAP * 2.0) / 3.0;
    let top_h = (body.h * 0.5).max(260.0);
    let col = |k: f32| Rect::new(body.x + k * (cw + GAP), body.y, cw, top_h);
    // income
    let r = col(0.0);
    l.ui.panel(r);
    let inner = l.ui.heading(r.pad(18.0, 12.0), "Income (30 days)", None);
    let mut y = inner.y;
    let mut sum_i = 0.0;
    for (k, v) in kinds.iter().filter(|(k, v)| *v > 0.0 && k.is_running()) {
        l.ui.text_in(k.label(), Rect::new(inner.x, y, inner.w * 0.65, 26.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
        l.ui.text_in(&money(*v), Rect::new(inner.x + inner.w * 0.35, y, inner.w * 0.65, 26.0), 13.0, Weight::Medium, TEXT, Align::Right);
        sum_i += v;
        y += 28.0;
    }
    l.ui.p().rect(Rect::new(inner.x, y + 4.0, inner.w, 1.0), EDGE);
    l.ui.text_in("Total", Rect::new(inner.x, y + 10.0, inner.w * 0.5, 26.0), 13.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in(&money(sum_i), Rect::new(inner.x + inner.w * 0.5, y + 10.0, inner.w * 0.5, 26.0), 13.0, Weight::Bold, OK, Align::Right);
    // costs
    let r = col(1.0);
    l.ui.panel(r);
    let inner = l.ui.heading(r.pad(18.0, 12.0), "Costs (30 days)", None);
    let mut y = inner.y;
    let mut sum_e = 0.0;
    for (k, v) in kinds.iter().filter(|(k, v)| *v < 0.0 && k.is_running()) {
        l.ui.text_in(k.label(), Rect::new(inner.x, y, inner.w * 0.65, 26.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
        l.ui.text_in(&money(-*v), Rect::new(inner.x + inner.w * 0.35, y, inner.w * 0.65, 26.0), 13.0, Weight::Medium, TEXT, Align::Right);
        sum_e -= v;
        y += 28.0;
    }
    l.ui.p().rect(Rect::new(inner.x, y + 4.0, inner.w, 1.0), EDGE);
    l.ui.text_in("Total", Rect::new(inner.x, y + 10.0, inner.w * 0.5, 26.0), 13.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in(&money(sum_e), Rect::new(inner.x + inner.w * 0.5, y + 10.0, inner.w * 0.5, 26.0), 13.0, Weight::Bold, DANGER, Align::Right);
    // the result, buses bought and sold, and closing the company
    let r = col(2.0);
    l.ui.panel(r);
    let inner = l.ui.heading(r.pad(18.0, 12.0), "Result (30 days)", None);
    let profit = sum_i - sum_e;
    l.ui.text_in(&format!("{}{}", if profit >= 0.0 { "+" } else { "" }, money(profit)), Rect::new(inner.x, inner.y + 4.0, inner.w, 40.0), 30.0, Weight::Black, if profit >= 0.0 { OK } else { DANGER }, Align::Left);
    let bought: f64 = kinds.iter().filter(|(k, _)| *k == company::Kind::Purchase).map(|x| -x.1).sum();
    let sold: f64 = kinds.iter().filter(|(k, _)| *k == company::Kind::Sale).map(|x| x.1).sum();
    l.ui.text_in(&format!("{}: {}", tr("Buses bought"), money(bought)), Rect::new(inner.x, inner.y + 56.0, inner.w, 20.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    l.ui.text_in(&format!("{}: {}", tr("Buses sold"), money(sold)), Rect::new(inner.x, inner.y + 80.0, inner.w, 20.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    l.ui.text_in(&format!("{}: {}", tr("Balance"), money(c.balance)), Rect::new(inner.x, inner.y + 104.0, inner.w, 20.0), 12.5, Weight::Bold, TEXT, Align::Left);
    if l.ui.button("co-values", Rect::new(inner.x, r.bottom() - 18.0 - ROW * 2.0 - 10.0, inner.w, ROW), "Edit the values (prices, diesel, wages)", Some("tune"), ButtonKind::Normal) {
        open_file(&core::company_values_path());
        l.state.set_status(tr("Change a number, save, then open this page again"), false);
    }
    let sure = l.company.armed("co-close");
    if l.ui.button("co-close", Rect::new(inner.x, r.bottom() - 18.0 - ROW, inner.w, ROW), if sure { "Click again: the company is gone for good" } else { "Close the company" }, Some("delete"), ButtonKind::Danger) {
        if sure {
            let _ = std::fs::remove_file(core::company_path());
            l.company.company = None;
            l.company.armed = None;
            l.state.set_status(tr("Company closed"), false);
            return;
        }
        l.company.arm("co-close");
    }
    // the bookings
    let book = Rect::new(body.x, body.y + top_h + GAP, body.w, body.h - top_h - GAP);
    l.ui.panel(book);
    let inner = l.ui.heading(book.pad(18.0, 12.0), "Bookings", None);
    let entries: Vec<company::Entry> = c.ledger.iter().rev().take(300).cloned().collect();
    l.ui.scroll_area("co-ledger", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h), &mut |ui, v| {
        let rh = 26.0;
        for (k, e) in entries.iter().enumerate() {
            let r = Rect::new(v.x + 6.0, v.y + k as f32 * rh, v.w - 12.0, rh);
            if !ui.rect_visible(r) {
                continue;
            }
            let d = (e.hours / company::hours_per_day()) as u32 + 1;
            ui.text_in(&format!("{} {d}", tr("Day")), Rect::new(r.x, r.y, 70.0, rh), 12.0, Weight::Regular, TEXT_FAINT, Align::Left);
            ui.text_in(e.kind.label(), Rect::new(r.x + 76.0, r.y, r.w * 0.25, rh), 12.0, Weight::Medium, TEXT_SOFT, Align::Left);
            ui.text_in(&entry_text(&e.text), Rect::new(r.x + 76.0 + r.w * 0.25, r.y, r.w * 0.5, rh), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(&money(e.amount), Rect::new(r.right() - 140.0, r.y, 140.0, rh), 12.5, Weight::Bold, if e.amount >= 0.0 { OK } else { DANGER }, Align::Right);
        }
        entries.len() as f32 * rh
    });
}

/// Open a text file for editing (Notepad on Windows, the system's choice elsewhere).
fn open_file(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("notepad.exe").arg(path).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    crate::updater::open_url(&path.to_string_lossy());
}

// --- the clicks -----------------------------------------------------------------------------

fn apply(l: &mut Launcher, acts: Vec<Act>) {
    if acts.is_empty() {
        return;
    }
    for a in acts {
        // (the dangerous ones need a second click)
        let key = match &a {
            Act::Sell(nr) => Some(format!("co-sell-{nr}")),
            Act::Drop(name) => Some(format!("co-drop-{name}")),
            Act::Fire(k) => Some(format!("co-fire-{k}")),
            _ => None,
        };
        if let Some(k) = key {
            if !l.company.armed(&k) {
                l.company.arm(&k);
                continue;
            }
            l.company.armed = None;
        }
        let Some(c) = l.company.company.as_mut() else { return };
        let msg: Result<String, String> = match a {
            Act::Buy(file, name) => c.buy(&file, &name).map(|nr| format!("{}: {name} ({} {nr})", tr("Bought"), tr("fleet number"))),
            Act::Sell(nr) => c.sell(nr).map(|v| format!("{} {nr}: {}", tr("Sold bus"), money(v))),
            Act::Repair(nr) => c.repair(nr).map(|v| format!("{} {nr}: {}", tr("Workshop for bus"), money(v))),
            Act::AssignBus(nr, line) => {
                c.assign_bus(nr, &line);
                Ok(String::new())
            }
            Act::Take(line) => {
                c.take_line(&line);
                Ok(format!("{} {line}", tr("Line taken over:")))
            }
            Act::Drop(line) => {
                c.drop_line(&line);
                Ok(format!("{} {line}", tr("Line given back:")))
            }
            Act::Hire(k) => match l.company.candidates.get(k).cloned() {
                Some(h) => {
                    let name = h.name.clone();
                    c.hire(h);
                    l.company.seed = l.company.seed.wrapping_add(7);
                    l.company.candidates = company::candidates(l.company.seed);
                    Ok(format!("{}: {name}", tr("Hired")))
                }
                None => Ok(String::new()),
            },
            Act::Fire(k) => {
                c.fire(k);
                Ok(tr("Driver dismissed"))
            }
            Act::AssignDriver(k, line) => {
                c.assign_driver(k, &line);
                Ok(String::new())
            }
        };
        match msg {
            Ok(m) if !m.is_empty() => l.state.set_status(m, false),
            Ok(_) => {}
            Err(e) => l.state.set_status(tr(&e), true),
        }
    }
    l.company.save(&mut l.state);
}
