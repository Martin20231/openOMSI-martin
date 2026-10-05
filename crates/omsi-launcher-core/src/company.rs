//! The bus company: a player's own transport business across duties.
//!
//! A company has a home map and depot, money, buses bought from the installed vehicles, the
//! lines of its map it took over and drivers it hired. It lives in
//! `~/.openomsi/company.cfg`, a plain `key=value` file in sections, and moves on only when
//! the player drives: each run the game writes into `~/.openomsi/sessions/` is booked once
//! ([`Company::book`]) - its ticket takings and the punctual stops are income, the diesel,
//! the wear of the bus and the crashes are costs - and the time it took is the company's
//! time ([`Company::run`]), in which the hired drivers drive the company's other buses on
//! its lines, the wages and the depot rent are paid and the city pays for each line served.
//!
//! Everything here is plain data and arithmetic (no files but [`Company::load`] and
//! [`Company::save`]), so the launcher's page only shows it and calls its actions.

use std::path::Path;

/// How hard the business is: the starting money and the prices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Easy, Level::Normal, Level::Hard];

    pub fn index(self) -> usize {
        match self {
            Level::Easy => 0,
            Level::Normal => 1,
            Level::Hard => 2,
        }
    }

    pub fn from_index(i: usize) -> Level {
        Level::ALL[i.min(2)]
    }

    pub fn name(self) -> &'static str {
        ["Easy", "Normal", "Hard"][self.index()]
    }

    /// What the company starts with.
    pub fn start_money(self) -> f64 {
        tuning().levels[self.index()].start_money
    }

    fn rules(self) -> Rules {
        tuning().levels[self.index()]
    }
}

/// The money and wear of a level (see [`Tuning`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    pub start_money: f64,
    /// € a litre.
    pub diesel: f64,
    /// Condition points a bus loses per 1000 km.
    pub wear_per_1000km: f64,
    /// € for each stop served on time.
    pub stop_bonus: f64,
    pub crash_fine: f64,
    pub hurt_fine: f64,
    pub rent_per_day: f64,
    /// What the city pays a day for each line the company serves.
    pub subsidy_per_line_day: f64,
    /// Takings an hour of a hired driver on a line, at full skill.
    pub ai_per_hour: f64,
    /// € an hour for a bus the company does not own (the player drove another one).
    pub rental_per_hour: f64,
}

/// Every number of the business, read from `company-values.cfg` beside the company's file
/// (written with these defaults when missing), so that the player can balance the game
/// without building it again.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub levels: [Rules; 3],
    /// Hours of driving that make one company day (its clock runs on the time the player
    /// drives, not on the calendar).
    pub hours_per_day: f64,
    /// Litres a bus burns a kilometre.
    pub litres_per_km: f64,
    /// How fast a hired driver's bus covers its line, on average (km/h).
    pub ai_speed: f64,
    /// Condition points a crash costs the bus.
    pub crash_wear: f64,
    /// New prices: a plain bus, a double decker, an articulated bus, a minibus.
    pub price_solo: f64,
    pub price_double: f64,
    pub price_articulated: f64,
    pub price_small: f64,
    /// The workshop asks this share of the price for a bus worn down to nothing.
    pub workshop_share: f64,
    /// A sold bus fetches this share of its price (less its wear).
    pub resale_share: f64,
    /// At most this many of the company's buses stand on its depot in the game.
    pub depot_buses: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        #[allow(clippy::too_many_arguments)]
        fn lv(start_money: f64, diesel: f64, wear_per_1000km: f64, stop_bonus: f64, crash_fine: f64, hurt_fine: f64, rent_per_day: f64, subsidy_per_line_day: f64, ai_per_hour: f64, rental_per_hour: f64) -> Rules {
            Rules { start_money, diesel, wear_per_1000km, stop_bonus, crash_fine, hurt_fine, rent_per_day, subsidy_per_line_day, ai_per_hour, rental_per_hour }
        }
        Tuning {
            levels: [
                lv(500_000.0, 1.20, 2.0, 1.00, 500.0, 2_000.0, 200.0, 300.0, 60.0, 40.0),
                lv(250_000.0, 1.50, 3.0, 0.60, 1_000.0, 4_000.0, 400.0, 200.0, 45.0, 60.0),
                lv(100_000.0, 1.90, 4.0, 0.40, 1_500.0, 6_000.0, 600.0, 100.0, 35.0, 90.0),
            ],
            hours_per_day: 2.0,
            litres_per_km: 0.42,
            ai_speed: 22.0,
            crash_wear: 5.0,
            price_solo: 160_000.0,
            price_double: 195_000.0,
            price_articulated: 240_000.0,
            price_small: 90_000.0,
            workshop_share: 0.25,
            resale_share: 0.5,
            depot_buses: 12.0,
        }
    }
}

/// The level sections and the keys of the values file (the file is the player's: German).
const LEVEL_KEYS: [&str; 3] = ["leicht", "normal", "schwer"];
const RULE_KEYS: [(&str, &str); 10] = [
    ("startgeld", "Startgeld (€)"),
    ("diesel", "Dieselpreis (€ pro Liter)"),
    ("verschleiss_pro_1000km", "Verschleiß in Prozentpunkten pro 1000 km"),
    ("puenktlich_bonus", "Bonus pro pünktlicher Haltestelle (€)"),
    ("strafe_unfall", "Strafe pro Unfall (€)"),
    ("strafe_fussgaenger", "Strafe pro verletztem Fußgänger (€)"),
    ("depotmiete_pro_tag", "Depotmiete pro Firmentag (€)"),
    ("zuschuss_pro_linie_tag", "Zuschuss der Stadt pro Linie mit Bus und Firmentag (€)"),
    ("ki_einnahmen_pro_stunde", "Fahrkarten eines KI-Fahrers pro Stunde bei vollem Können (€)"),
    ("leihbus_pro_stunde", "Leihgebühr pro Stunde für einen Bus, der nicht der Firma gehört (€)"),
];
const GENERAL_KEYS: [(&str, &str); 11] = [
    ("stunden_pro_tag", "So viele Stunden Fahrzeit sind ein Firmentag"),
    ("liter_pro_km", "Dieselverbrauch eines Busses (Liter pro km)"),
    ("ki_tempo", "Durchschnittstempo der KI-Busse auf ihrer Linie (km/h)"),
    ("unfall_verschleiss", "Verschleiß pro Unfall (Prozentpunkte)"),
    ("preis_solobus", "Neupreis eines Solobusses (€)"),
    ("preis_doppeldecker", "Neupreis eines Doppeldeckers (€)"),
    ("preis_gelenkbus", "Neupreis eines Gelenkbusses (€)"),
    ("preis_kleinbus", "Neupreis eines Kleinbusses (€)"),
    ("werkstatt_anteil", "Werkstatt: dieser Anteil vom Neupreis für einen ganz verschlissenen Bus"),
    ("verkauf_anteil", "Verkauf: dieser Anteil vom Neupreis (abzüglich Verschleiß)"),
    ("betriebshof_max_busse", "So viele Firmenbusse stehen im Spiel höchstens auf dem Betriebshof"),
];

impl Rules {
    fn slot(&mut self, k: usize) -> &mut f64 {
        match k {
            0 => &mut self.start_money,
            1 => &mut self.diesel,
            2 => &mut self.wear_per_1000km,
            3 => &mut self.stop_bonus,
            4 => &mut self.crash_fine,
            5 => &mut self.hurt_fine,
            6 => &mut self.rent_per_day,
            7 => &mut self.subsidy_per_line_day,
            8 => &mut self.ai_per_hour,
            _ => &mut self.rental_per_hour,
        }
    }
}

impl Tuning {
    fn slot(&mut self, k: usize) -> &mut f64 {
        match k {
            0 => &mut self.hours_per_day,
            1 => &mut self.litres_per_km,
            2 => &mut self.ai_speed,
            3 => &mut self.crash_wear,
            4 => &mut self.price_solo,
            5 => &mut self.price_double,
            6 => &mut self.price_articulated,
            7 => &mut self.price_small,
            8 => &mut self.workshop_share,
            9 => &mut self.resale_share,
            _ => &mut self.depot_buses,
        }
    }

    /// The values file's text: every value with a line saying what it is.
    pub fn to_text(&self) -> String {
        let mut c = self.clone();
        let mut s = String::from("# openOMSI – Werte für das Unternehmen.\n# Eine Zahl ändern, speichern, dann im Launcher die Seite „Unternehmen“ neu öffnen.\n# Kommazahlen mit Punkt schreiben (1.5). Eine gelöschte Zeile bekommt wieder ihren Standardwert.\n");
        for (i, sec) in LEVEL_KEYS.iter().enumerate() {
            s.push_str(&format!("\n[{sec}]\n"));
            for (k, (key, what)) in RULE_KEYS.iter().enumerate() {
                s.push_str(&format!("# {what}\n{key}={}\n", fmt_num(*c.levels[i].slot(k))));
            }
        }
        s.push_str("\n[allgemein]\n");
        for (k, (key, what)) in GENERAL_KEYS.iter().enumerate() {
            s.push_str(&format!("# {what}\n{key}={}\n", fmt_num(*c.slot(k))));
        }
        s
    }

    /// The values from a file's text; anything missing or unreadable keeps its default.
    pub fn parse(text: &str) -> Tuning {
        let mut t = Tuning::default();
        let mut section = String::new();
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if l.starts_with('[') && l.ends_with(']') {
                section = l[1..l.len() - 1].trim().to_lowercase();
                continue;
            }
            let Some((k, v)) = l.split_once('=') else { continue };
            let (k, v) = (k.trim().to_lowercase(), v.trim().replace(',', "."));
            let Ok(v) = v.parse::<f64>() else { continue };
            if !v.is_finite() || v < 0.0 {
                continue;
            }
            if let Some(i) = LEVEL_KEYS.iter().position(|s| *s == section) {
                if let Some(j) = RULE_KEYS.iter().position(|(key, _)| *key == k) {
                    *t.levels[i].slot(j) = v;
                }
            } else if section == "allgemein" {
                if let Some(j) = GENERAL_KEYS.iter().position(|(key, _)| *key == k) {
                    *t.slot(j) = v;
                }
            }
        }
        // (a day of no hours would never end)
        t.hours_per_day = t.hours_per_day.max(0.1);
        t
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

static TUNING: std::sync::RwLock<Option<Tuning>> = std::sync::RwLock::new(None);

/// The values in force (the file's, once [`load_tuning`] read it; else the defaults).
pub fn tuning() -> Tuning {
    TUNING.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Read the values file at `path`, writing it with the defaults when there is none.
pub fn load_tuning(path: &Path) -> Result<(), String> {
    let t = match std::fs::read_to_string(path) {
        Ok(text) => Tuning::parse(&text),
        Err(_) => {
            let t = Tuning::default();
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            std::fs::write(path, t.to_text()).map_err(|e| format!("{}: {e}", path.display()))?;
            t
        }
    };
    if let Ok(mut g) = TUNING.write() {
        *g = Some(t);
    }
    Ok(())
}

/// Hours of driving that make one company day.
pub fn hours_per_day() -> f64 {
    tuning().hours_per_day
}

/// Below this condition a bus should go to the workshop.
pub const WORKSHOP_BELOW: f64 = 40.0;
/// The colours a company may take (RGB).
pub const COLORS: [(&str, u32); 6] = [("Orange", 0xF2A33A), ("Yellow", 0xF5D04C), ("Red", 0xE5534B), ("Blue", 0x4C8DF5), ("Green", 0x4CC38A), ("White", 0xECECEC)];

/// A bus of the company.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bus {
    /// Fleet number.
    pub nr: u32,
    /// The vehicle's `.bus` file as the launcher lists it.
    pub file: String,
    pub name: String,
    /// What it cost new.
    pub price: f64,
    /// 0 … 100.
    pub condition: f64,
    pub km: f64,
    /// The line it is on ("" = in the depot).
    pub line: String,
}

/// A driver the company hired.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hire {
    pub name: String,
    /// 0 … 1: punctuality and care of the bus.
    pub skill: f64,
    /// € a month (30 company days).
    pub wage: f64,
    /// The line the driver works ("" = none).
    pub line: String,
}

/// What a booking was for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tickets,
    Bonus,
    Subsidy,
    AiTakings,
    Diesel,
    Wages,
    Workshop,
    Rent,
    Fines,
    Rental,
    Purchase,
    Sale,
    Start,
}

impl Kind {
    const ALL: [Kind; 13] = [Kind::Tickets, Kind::Bonus, Kind::Subsidy, Kind::AiTakings, Kind::Diesel, Kind::Wages, Kind::Workshop, Kind::Rent, Kind::Fines, Kind::Rental, Kind::Purchase, Kind::Sale, Kind::Start];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Tickets => "tickets",
            Kind::Bonus => "bonus",
            Kind::Subsidy => "subsidy",
            Kind::AiTakings => "ai",
            Kind::Diesel => "diesel",
            Kind::Wages => "wages",
            Kind::Workshop => "workshop",
            Kind::Rent => "rent",
            Kind::Fines => "fines",
            Kind::Rental => "rental",
            Kind::Purchase => "purchase",
            Kind::Sale => "sale",
            Kind::Start => "start",
        }
    }

    /// The English label (the launcher translates it).
    pub fn label(self) -> &'static str {
        match self {
            Kind::Tickets => "Tickets",
            Kind::Bonus => "Punctuality bonus",
            Kind::Subsidy => "City subsidy",
            Kind::AiTakings => "Tickets of hired drivers",
            Kind::Diesel => "Diesel",
            Kind::Wages => "Wages",
            Kind::Workshop => "Workshop",
            Kind::Rent => "Depot rent",
            Kind::Fines => "Fines (crashes)",
            Kind::Rental => "Hired buses",
            Kind::Purchase => "Buses bought",
            Kind::Sale => "Buses sold",
            Kind::Start => "Starting capital",
        }
    }

    fn from_key(k: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|x| x.key() == k)
    }

    /// Buying and selling buses move money but are no running income or cost.
    pub fn is_running(self) -> bool {
        !matches!(self, Kind::Purchase | Kind::Sale | Kind::Start)
    }
}

/// One booking: when (company hours), what, how much (+ income, − cost), and a line of text.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub hours: f64,
    pub kind: Kind,
    pub amount: f64,
    pub text: String,
}

/// A run of the player as the game wrote it (`sessions/*.json`), the part the company needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    /// Unix time the run ended (its id).
    pub time: u64,
    pub map: String,
    pub bus: String,
    pub line: Option<String>,
    pub seconds: f64,
    pub metres: f64,
    pub stops: i32,
    pub early: i32,
    pub late: i32,
    pub cash: f64,
    pub crashes: i32,
    pub hurt: i32,
    /// A friend drove it for the company in a multiplayer session (their name): no rent
    /// for their bus, and the company's own time does not move on with it (the host's
    /// run, at the same time, moves it).
    pub driver: Option<String>,
}

/// What booking a run did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Booked {
    pub income: f64,
    pub costs: f64,
    /// The company's bus that was driven (fleet number), if it was one of them.
    pub bus: Option<u32>,
}

/// What a run brings and costs (see [`Company::price`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunPrice {
    pub km: f64,
    pub hours: f64,
    pub punctual: i32,
    pub tickets: f64,
    pub bonus: f64,
    pub diesel: f64,
    /// For a bus the company does not own.
    pub rental: f64,
    pub fines: f64,
    /// The company's bus driven (index in `buses`).
    pub own_bus: Option<usize>,
}

impl RunPrice {
    pub fn income(&self) -> f64 {
        self.tickets + self.bonus
    }

    pub fn costs(&self) -> f64 {
        self.diesel + self.rental + self.fines
    }

    pub fn result(&self) -> f64 {
        self.income() - self.costs()
    }
}

/// A trip the player drove for the company (the last ones are kept for the overview).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Trip {
    pub time: u64,
    pub line: String,
    pub bus: String,
    pub km: f64,
    pub result: f64,
    pub punctual: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Company {
    pub name: String,
    pub short: String,
    /// Index into [`COLORS`].
    pub color: usize,
    /// The home map (its `global.cfg` as the launcher lists it).
    pub map: String,
    pub depot: String,
    pub level: Level,
    pub balance: f64,
    /// Unix time it was founded: runs before it are not its.
    pub founded: u64,
    /// Company hours (see [`hours_per_day`]).
    pub hours: f64,
    pub next_nr: u32,
    pub buses: Vec<Bus>,
    pub lines: Vec<String>,
    pub drivers: Vec<Hire>,
    pub ledger: Vec<Entry>,
    /// The runs booked already (their times).
    pub seen: Vec<u64>,
    pub trips: Vec<Trip>,
    /// Stops served by the player, of those on time.
    pub stops: i64,
    pub on_time: i64,
    /// Shared with other players (multiplayer; the next stage).
    pub shared: bool,
}

/// How many bookings and trips the file keeps (the oldest go).
const KEEP_ENTRIES: usize = 2_000;
const KEEP_TRIPS: usize = 30;

/// What a new bus of this kind costs (a guess from its name: double deckers and articulated
/// buses cost more).
pub fn price_for(name: &str, file: &str) -> f64 {
    let n = format!("{} {}", name, file).to_ascii_lowercase();
    let t = tuning();
    if ["sd2", "doppeldeck", "double", "dd ", "_dd", "lion's city dd"].iter().any(|k| n.contains(k)) {
        t.price_double
    } else if ["gelenk", "articul", "ng2", "ng3", " g ", "_g.", "citaro g", "capacity", "18m", "18 m"].iter().any(|k| n.contains(k)) {
        t.price_articulated
    } else if ["sprinter", "midi", "minibus", "vario"].iter().any(|k| n.contains(k)) {
        t.price_small
    } else {
        t.price_solo
    }
}

/// Drivers one may hire: (name, skill, wage a month).
pub fn candidates(seed: u64) -> Vec<Hire> {
    const FIRST: [&str; 12] = ["Jens", "Ayla", "Marco", "Sabine", "Tim", "Olga", "Kemal", "Laura", "Piotr", "Nina", "Uwe", "Sara"];
    const LAST: [&str; 12] = ["Weber", "Yilmaz", "Brandt", "Koch", "Schulz", "Nowak", "Arslan", "Becker", "Lange", "Fischer", "Krüger", "Hoffmann"];
    let tiers = [(0.50, 2_600.0), (0.80, 3_100.0), (0.95, 3_600.0)];
    (0..3)
        .map(|k| {
            let i = (seed.wrapping_mul(2654435761).wrapping_add(k * 7919) % 144) as usize;
            let (skill, wage) = tiers[k as usize];
            Hire { name: format!("{} {}", FIRST[i % 12], LAST[(i / 12) % 12]), skill, wage, line: String::new() }
        })
        .collect()
}

impl Company {
    /// A new company with its starting money.
    pub fn found(name: &str, short: &str, color: usize, map: &str, depot: &str, level: Level, now: u64) -> Company {
        let mut c = Company {
            name: name.trim().to_string(),
            short: short.trim().to_string(),
            color: color.min(COLORS.len() - 1),
            map: map.to_string(),
            depot: depot.to_string(),
            level,
            founded: now,
            next_nr: 1001,
            ..Default::default()
        };
        c.book_entry(Kind::Start, level.start_money(), "");
        c
    }

    fn rules(&self) -> Rules {
        self.level.rules()
    }

    /// Company days so far.
    pub fn day(&self) -> u32 {
        (self.hours / hours_per_day()) as u32 + 1
    }

    fn book_entry(&mut self, kind: Kind, amount: f64, text: &str) {
        if amount.abs() < 0.005 {
            return;
        }
        self.balance += amount;
        self.ledger.push(Entry { hours: self.hours, kind, amount: (amount * 100.0).round() / 100.0, text: text.to_string() });
        if self.ledger.len() > KEEP_ENTRIES {
            let cut = self.ledger.len() - KEEP_ENTRIES;
            self.ledger.drain(..cut);
        }
    }

    /// The run is on this company's map and after its founding, and not booked yet.
    pub fn takes(&self, run: &Run) -> bool {
        run.time >= self.founded && !self.seen.contains(&run.time) && same_file(&run.map, &self.map)
    }

    /// Book a run of the player: its takings and costs, then the company time it took.
    /// What a run brings and costs, by its parts (nothing is booked): what [`Company::book`]
    /// books, and what the game shows while the run is still going.
    pub fn price(&self, run: &Run) -> RunPrice {
        let r = self.rules();
        let tune = tuning();
        let km = (run.metres / 1000.0).max(0.0);
        let hours = (run.seconds / 3600.0).max(0.0);
        let punctual = (run.stops - run.early - run.late).max(0);
        let own_bus = self.buses.iter().position(|b| same_file(&b.file, &run.bus));
        RunPrice {
            km,
            hours,
            punctual,
            tickets: run.cash.max(0.0),
            bonus: punctual as f64 * r.stop_bonus,
            diesel: km * tune.litres_per_km * r.diesel,
            rental: if own_bus.is_none() && run.driver.is_none() { hours * r.rental_per_hour } else { 0.0 },
            fines: run.crashes.max(0) as f64 * r.crash_fine + run.hurt.max(0) as f64 * r.hurt_fine,
            own_bus,
        }
    }

    /// Book a run of the player: its takings and costs, then the company time it took.
    pub fn book(&mut self, run: &Run) -> Option<Booked> {
        if !self.takes(run) {
            return None;
        }
        self.seen.push(run.time);
        let p = self.price(run);
        let r = self.rules();
        let tune = tuning();
        let line = run.line.clone().unwrap_or_default();
        let what = if line.is_empty() { "Free drive".to_string() } else { format!("Line {line}") };
        let mut out = Booked::default();
        let before = self.balance;
        self.book_entry(Kind::Tickets, p.tickets, &what);
        self.stops += run.stops.max(0) as i64;
        self.on_time += p.punctual as i64;
        self.book_entry(Kind::Bonus, p.bonus, &format!("{what} · {}/{}", p.punctual, run.stops));
        self.book_entry(Kind::Diesel, -p.diesel, &format!("{what} · {:.1} km", p.km));
        self.book_entry(Kind::Rental, -p.rental, &what);
        self.book_entry(Kind::Fines, -p.fines, &format!("{what} · {}", run.crashes + run.hurt));
        if let Some(i) = p.own_bus {
            let b = &mut self.buses[i];
            b.km += p.km;
            b.condition = (b.condition - p.km / 1000.0 * r.wear_per_1000km - run.crashes.max(0) as f64 * tune.crash_wear).clamp(0.0, 100.0);
            out.bus = Some(b.nr);
        }
        let mut bus_name = run.bus.rsplit(['/', '\\']).next().unwrap_or(&run.bus).trim_end_matches(".bus").to_string();
        if let Some(d) = &run.driver {
            bus_name = format!("{bus_name} · {d}");
        }
        self.trips.insert(0, Trip { time: run.time, line, bus: bus_name, km: p.km, result: self.balance - before, punctual: if run.stops > 0 { p.punctual as f64 / run.stops as f64 } else { 1.0 } });
        self.trips.truncate(KEEP_TRIPS);
        out.income = p.income();
        out.costs = p.costs();
        // the time it took: the rest of the company works meanwhile (a friend's run went at
        // the same time as the host's)
        if run.driver.is_none() {
            self.run(p.hours);
        }
        Some(out)
    }

    /// `hours` of company time: the hired drivers drive the buses on their lines, and the
    /// wages, the rent and the city's money for the lines served are booked.
    pub fn run(&mut self, hours: f64) {
        // (a company day at a time, so that each day's bookings are dated on it)
        let mut left = hours;
        while left > 1e-9 {
            let hpd = hours_per_day();
            let step = left.min(hpd - self.hours.rem_euclid(hpd)).max(1e-6).min(left);
            self.run_step(step);
            left -= step;
        }
    }

    fn run_step(&mut self, hours: f64) {
        if hours <= 0.0 {
            return;
        }
        let r = self.rules();
        let tune = tuning();
        let days = hours / tune.hours_per_day;
        // hired drivers: each needs a bus of its line
        let mut used: Vec<usize> = Vec::new();
        let mut takings = 0.0;
        let mut diesel = 0.0;
        let mut trips = 0;
        for d in self.drivers.clone().iter().filter(|d| !d.line.is_empty()) {
            let Some(bi) = self.buses.iter().enumerate().position(|(i, b)| b.line == d.line && !used.contains(&i) && b.condition > 0.0) else { continue };
            used.push(bi);
            let km = hours * tune.ai_speed;
            // a bus in a bad state breaks down now and then: less takings
            let state = (self.buses[bi].condition / 100.0).clamp(0.2, 1.0);
            takings += hours * r.ai_per_hour * (0.6 + 0.4 * d.skill) * (0.5 + 0.5 * state);
            diesel += km * tune.litres_per_km * r.diesel;
            let b = &mut self.buses[bi];
            b.km += km;
            // (a good driver is kind to the bus)
            b.condition = (b.condition - km / 1000.0 * r.wear_per_1000km * (1.4 - 0.6 * d.skill)).clamp(0.0, 100.0);
            trips += 1;
        }
        if takings > 0.0 {
            self.book_entry(Kind::AiTakings, takings, &format!("{trips} ×"));
            self.book_entry(Kind::Diesel, -diesel, &format!("{trips} ×"));
        }
        let wages: f64 = self.drivers.iter().map(|d| d.wage).sum::<f64>() * days / 30.0;
        self.book_entry(Kind::Wages, -wages, "");
        self.book_entry(Kind::Rent, -r.rent_per_day * days, "");
        let served = self.lines.iter().filter(|l| self.buses.iter().any(|b| &b.line == *l)).count();
        self.book_entry(Kind::Subsidy, r.subsidy_per_line_day * served as f64 * days, &format!("{served} ×"));
        self.hours += hours;
    }

    /// Buy a new bus (`file`, `name` as the launcher lists the vehicle).
    pub fn buy(&mut self, file: &str, name: &str) -> Result<u32, String> {
        let price = price_for(name, file);
        if self.balance < price {
            return Err("Not enough money for this bus".into());
        }
        let nr = self.next_nr.max(1001);
        self.next_nr = nr + 1;
        self.buses.push(Bus { nr, file: file.to_string(), name: name.to_string(), price, condition: 100.0, km: 0.0, line: String::new() });
        self.book_entry(Kind::Purchase, -price, &format!("{name} ({nr})"));
        Ok(nr)
    }

    /// What a bus fetches when sold: half its price, less its wear.
    pub fn resale(&self, nr: u32) -> Option<f64> {
        let b = self.buses.iter().find(|b| b.nr == nr)?;
        Some((b.price * tuning().resale_share * (0.3 + 0.7 * b.condition / 100.0)).round())
    }

    pub fn sell(&mut self, nr: u32) -> Result<f64, String> {
        let v = self.resale(nr).ok_or("No such bus")?;
        let b = self.buses.iter().position(|b| b.nr == nr).ok_or("No such bus")?;
        let b = self.buses.remove(b);
        self.book_entry(Kind::Sale, v, &format!("{} ({})", b.name, b.nr));
        Ok(v)
    }

    /// What the workshop asks to make a bus as good as new.
    pub fn repair_cost(&self, nr: u32) -> Option<f64> {
        let b = self.buses.iter().find(|b| b.nr == nr)?;
        Some((b.price * (100.0 - b.condition) / 100.0 * tuning().workshop_share).round())
    }

    pub fn repair(&mut self, nr: u32) -> Result<f64, String> {
        let cost = self.repair_cost(nr).ok_or("No such bus")?;
        if cost <= 0.0 {
            return Ok(0.0);
        }
        if self.balance < cost {
            return Err("Not enough money for the workshop".into());
        }
        let b = self.buses.iter_mut().find(|b| b.nr == nr).ok_or("No such bus")?;
        b.condition = 100.0;
        let text = format!("{} ({})", b.name, b.nr);
        self.book_entry(Kind::Workshop, -cost, &text);
        Ok(cost)
    }

    /// Put a bus on a line ("" = back to the depot).
    pub fn assign_bus(&mut self, nr: u32, line: &str) {
        if let Some(b) = self.buses.iter_mut().find(|b| b.nr == nr) {
            b.line = line.to_string();
        }
    }

    pub fn take_line(&mut self, line: &str) {
        if !self.lines.iter().any(|l| l == line) {
            self.lines.push(line.to_string());
        }
    }

    /// Give a line back: its buses and drivers go to the depot.
    pub fn drop_line(&mut self, line: &str) {
        self.lines.retain(|l| l != line);
        for b in self.buses.iter_mut().filter(|b| b.line == line) {
            b.line.clear();
        }
        for d in self.drivers.iter_mut().filter(|d| d.line == line) {
            d.line.clear();
        }
    }

    pub fn hire(&mut self, h: Hire) {
        self.drivers.push(h);
    }

    pub fn fire(&mut self, index: usize) {
        if index < self.drivers.len() {
            self.drivers.remove(index);
        }
    }

    pub fn assign_driver(&mut self, index: usize, line: &str) {
        if let Some(d) = self.drivers.get_mut(index) {
            d.line = line.to_string();
        }
    }

    /// The share of the player's stops served on time (1 with none yet).
    pub fn punctuality(&self) -> f64 {
        if self.stops > 0 { self.on_time as f64 / self.stops as f64 } else { 1.0 }
    }

    /// Running income and costs (no purchases) of the company days `from..to`.
    pub fn sums(&self, from_day: u32, to_day: u32) -> (f64, f64) {
        let hpd = hours_per_day();
        let (a, b) = ((from_day.max(1) - 1) as f64 * hpd, (to_day.max(1) - 1) as f64 * hpd);
        let mut inc = 0.0;
        let mut exp = 0.0;
        for e in self.ledger.iter().filter(|e| e.kind.is_running() && e.hours >= a && e.hours < b) {
            if e.amount >= 0.0 { inc += e.amount } else { exp -= e.amount }
        }
        (inc, exp)
    }

    /// Income and costs of each of the last `n` company weeks (7 days), oldest first.
    pub fn weeks(&self, n: u32) -> Vec<(f64, f64)> {
        let this = (self.day() - 1) / 7;
        (0..n)
            .rev()
            .map(|k| {
                let w = this as i64 - k as i64;
                if w < 0 {
                    return (0.0, 0.0);
                }
                let from = w as u32 * 7 + 1;
                self.sums(from, from + 7)
            })
            .collect()
    }

    /// Running income and costs by kind over the company days `from..to`.
    pub fn by_kind(&self, from_day: u32, to_day: u32) -> Vec<(Kind, f64)> {
        let hpd = hours_per_day();
        let (a, b) = ((from_day.max(1) - 1) as f64 * hpd, (to_day.max(1) - 1) as f64 * hpd);
        let mut out: Vec<(Kind, f64)> = Vec::new();
        for e in self.ledger.iter().filter(|e| e.hours >= a && e.hours < b && e.kind != Kind::Start) {
            match out.iter_mut().find(|(k, _)| *k == e.kind) {
                Some(x) => x.1 += e.amount,
                None => out.push((e.kind, e.amount)),
            }
        }
        out
    }

    /// Things to look at: (severity 0 info … 2 urgent, the English text with `{}` where
    /// the value goes, the value) - the launcher translates the text, then puts the value in.
    pub fn notices(&self, free_lines: usize) -> Vec<(u8, &'static str, String)> {
        let mut out = Vec::new();
        if self.balance < 0.0 {
            out.push((2, "The account is in the red ({}): sell a bus or drive more", money(self.balance)));
        }
        for b in self.buses.iter().filter(|b| b.condition < WORKSHOP_BELOW) {
            out.push((2, "Bus {} needs the workshop", format!("{} ({:.0} %)", b.nr, b.condition)));
        }
        for l in self.lines.iter().filter(|l| !self.buses.iter().any(|b| &b.line == *l)) {
            out.push((1, "Line {} has no bus: put one on it in Fleet", l.clone()));
        }
        let idle = self.drivers.iter().filter(|d| d.line.is_empty()).count();
        if idle > 0 {
            out.push((1, "{} driver(s) without a line: give them one in Staff", idle.to_string()));
        }
        if self.buses.is_empty() {
            out.push((1, "Buy your first bus in Fleet", String::new()));
        }
        if free_lines > 0 && self.lines.is_empty() {
            out.push((0, "{} lines of the map are free: take one over in Lines", free_lines.to_string()));
        }
        out
    }

    // --- the file ----------------------------------------------------------------------

    /// The company as its file writes it.
    pub fn to_text(&self) -> String {
        let mut s = String::from("[company]\n");
        let kv = |s: &mut String, k: &str, v: &str| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v.replace(['\n', '\r'], " "));
            s.push('\n');
        };
        kv(&mut s, "version", "1");
        kv(&mut s, "name", &self.name);
        kv(&mut s, "short", &self.short);
        kv(&mut s, "color", &self.color.to_string());
        kv(&mut s, "map", &self.map);
        kv(&mut s, "depot", &self.depot);
        kv(&mut s, "level", &self.level.index().to_string());
        kv(&mut s, "balance", &format!("{:.2}", self.balance));
        kv(&mut s, "founded", &self.founded.to_string());
        kv(&mut s, "hours", &format!("{:.4}", self.hours));
        kv(&mut s, "next_nr", &self.next_nr.to_string());
        kv(&mut s, "stops", &self.stops.to_string());
        kv(&mut s, "on_time", &self.on_time.to_string());
        kv(&mut s, "shared", if self.shared { "1" } else { "0" });
        kv(&mut s, "lines", &self.lines.join("|"));
        kv(&mut s, "seen", &self.seen.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(","));
        for b in &self.buses {
            s.push_str("\n[bus]\n");
            kv(&mut s, "nr", &b.nr.to_string());
            kv(&mut s, "file", &b.file);
            kv(&mut s, "name", &b.name);
            kv(&mut s, "price", &format!("{:.2}", b.price));
            kv(&mut s, "condition", &format!("{:.3}", b.condition));
            kv(&mut s, "km", &format!("{:.3}", b.km));
            kv(&mut s, "line", &b.line);
        }
        for d in &self.drivers {
            s.push_str("\n[driver]\n");
            kv(&mut s, "name", &d.name);
            kv(&mut s, "skill", &format!("{:.3}", d.skill));
            kv(&mut s, "wage", &format!("{:.2}", d.wage));
            kv(&mut s, "line", &d.line);
        }
        for t in &self.trips {
            s.push_str("\n[trip]\n");
            kv(&mut s, "time", &t.time.to_string());
            kv(&mut s, "line", &t.line);
            kv(&mut s, "bus", &t.bus);
            kv(&mut s, "km", &format!("{:.3}", t.km));
            kv(&mut s, "result", &format!("{:.2}", t.result));
            kv(&mut s, "punctual", &format!("{:.4}", t.punctual));
        }
        s.push_str("\n[ledger]\n");
        for e in &self.ledger {
            s.push_str(&format!("{:.4};{};{:.2};{}\n", e.hours, e.kind.key(), e.amount, e.text.replace(['\n', '\r', ';'], " ")));
        }
        s
    }

    /// The company from its file's text.
    pub fn from_text(text: &str) -> Result<Company, String> {
        let mut c = Company::default();
        let mut section = String::new();
        let mut found = false;
        for raw in text.lines() {
            let line = raw.trim_end_matches('\r');
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                section = t[1..t.len() - 1].to_ascii_lowercase();
                match section.as_str() {
                    "company" => found = true,
                    "bus" => c.buses.push(Bus::default()),
                    "driver" => c.drivers.push(Hire::default()),
                    "trip" => c.trips.push(Trip::default()),
                    _ => {}
                }
                continue;
            }
            if section == "ledger" {
                let mut p = line.splitn(4, ';');
                let (Some(h), Some(k), Some(a)) = (p.next(), p.next(), p.next()) else { continue };
                let (Ok(hours), Some(kind), Ok(amount)) = (h.parse::<f64>(), Kind::from_key(k), a.parse::<f64>()) else { continue };
                c.ledger.push(Entry { hours, kind, amount, text: p.next().unwrap_or("").to_string() });
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let f = |v: &str| v.parse::<f64>().unwrap_or(0.0);
            match section.as_str() {
                "company" => match k {
                    "name" => c.name = v.to_string(),
                    "short" => c.short = v.to_string(),
                    "color" => c.color = v.parse::<usize>().unwrap_or(0).min(COLORS.len() - 1),
                    "map" => c.map = v.to_string(),
                    "depot" => c.depot = v.to_string(),
                    "level" => c.level = Level::from_index(v.parse().unwrap_or(1)),
                    "balance" => c.balance = f(v),
                    "founded" => c.founded = v.parse().unwrap_or(0),
                    "hours" => c.hours = f(v),
                    "next_nr" => c.next_nr = v.parse().unwrap_or(1001),
                    "stops" => c.stops = v.parse().unwrap_or(0),
                    "on_time" => c.on_time = v.parse().unwrap_or(0),
                    "shared" => c.shared = v == "1",
                    "lines" => c.lines = v.split('|').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
                    "seen" => c.seen = v.split(',').filter_map(|s| s.trim().parse().ok()).collect(),
                    _ => {}
                },
                "bus" => {
                    let Some(b) = c.buses.last_mut() else { continue };
                    match k {
                        "nr" => b.nr = v.parse().unwrap_or(0),
                        "file" => b.file = v.to_string(),
                        "name" => b.name = v.to_string(),
                        "price" => b.price = f(v),
                        "condition" => b.condition = f(v).clamp(0.0, 100.0),
                        "km" => b.km = f(v),
                        "line" => b.line = v.to_string(),
                        _ => {}
                    }
                }
                "driver" => {
                    let Some(d) = c.drivers.last_mut() else { continue };
                    match k {
                        "name" => d.name = v.to_string(),
                        "skill" => d.skill = f(v).clamp(0.0, 1.0),
                        "wage" => d.wage = f(v),
                        "line" => d.line = v.to_string(),
                        _ => {}
                    }
                }
                "trip" => {
                    let Some(t) = c.trips.last_mut() else { continue };
                    match k {
                        "time" => t.time = v.parse().unwrap_or(0),
                        "line" => t.line = v.to_string(),
                        "bus" => t.bus = v.to_string(),
                        "km" => t.km = f(v),
                        "result" => t.result = f(v),
                        "punctual" => t.punctual = f(v),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if !found || c.name.is_empty() {
            return Err("not a company file".into());
        }
        Ok(c)
    }

    pub fn load(path: &Path) -> Option<Company> {
        let text = std::fs::read_to_string(path).ok()?;
        Company::from_text(&text).ok()
    }

    /// Write it (through a temporary file, so a crash never leaves half a company).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("cfg.tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(&tmp, path)
    }
}

/// Two content paths name the same file (case, `\` or `/`, a leading content root aside).
pub fn same_file(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().replace('\\', "/").to_ascii_lowercase();
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && !b.is_empty() && (a == b || a.ends_with(&format!("/{b}")) || b.ends_with(&format!("/{a}")))
}

/// An amount as the launcher shows it: "248.350 €" (German grouping; whole euros).
pub fn money(v: f64) -> String {
    let n = v.abs().round() as u64;
    // (less than half a euro is 0 €, without a sign)
    let neg = v < 0.0 && n > 0;
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    format!("{}{} €", if neg { "−" } else { "" }, out)
}

/// An entry point's name without the number the launcher gives repeated ones ("Hof (2)").
pub fn depot_base(name: &str) -> &str {
    let n = name.trim();
    if let Some(open) = n.rfind(" (") {
        let inner = &n[open + 2..];
        if inner.ends_with(')') && inner.len() > 1 && inner[..inner.len() - 1].chars().all(|c| c.is_ascii_digit()) {
            return n[..open].trim_end();
        }
    }
    n
}

/// Words that name a depot in a map's entry points (German maps mostly).
const DEPOT_WORDS: [&str; 6] = ["betriebshof", "depot", "garage", "remise", "bvhof", "busbahnhof"];

/// The depots a map offers to choose from: its entry points' names once each, those that
/// sound like a depot first.
pub fn depot_names(entries: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in entries {
        let b = depot_base(e);
        if !b.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(b)) {
            out.push(b.to_string());
        }
    }
    let is_depot = |n: &str| DEPOT_WORDS.iter().any(|w| n.to_lowercase().contains(w));
    out.sort_by_key(|n| !is_depot(n));
    out
}

/// The entry points (their places in the list) of the depot named `depot`: those of that
/// name, else those that sound like a depot. Empty when the map has neither.
pub fn depot_entries(entries: &[String], depot: &str) -> Vec<usize> {
    let want = depot_base(depot).to_lowercase();
    let exact: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| !want.is_empty() && depot_base(e).to_lowercase() == want).map(|(i, _)| i).collect();
    if !exact.is_empty() {
        return exact;
    }
    // (the first depot-sounding name and the entries that share it)
    let first = entries.iter().map(|e| depot_base(e).to_lowercase()).find(|n| DEPOT_WORDS.iter().any(|w| n.contains(w)));
    match first {
        Some(f) => entries.iter().enumerate().filter(|(_, e)| depot_base(e).to_lowercase() == f).map(|(i, _)| i).collect(),
        None => Vec::new(),
    }
}

/// The company's buses that wait on its depot in the game, in the order they are put down:
/// not those on the road for its hired drivers (`on_lines`: with the timetable's buses), nor
/// the one the player drives (`driven`, the file); the worn ones (under 30 %) last, apart.
pub fn depot_buses<'a>(c: &'a Company, on_lines: bool, driven: Option<&str>) -> Vec<&'a Bus> {
    let mut busy: Vec<u32> = Vec::new();
    if on_lines {
        for line in &c.lines {
            let drivers = c.drivers.iter().filter(|d| &d.line == line).count();
            busy.extend(c.buses.iter().filter(|b| &b.line == line && b.condition > 0.0).take(drivers).map(|b| b.nr));
        }
    }
    let mut skip_driven = driven.filter(|d| !d.trim().is_empty());
    let mut out: Vec<&Bus> = Vec::new();
    for b in &c.buses {
        if busy.contains(&b.nr) {
            continue;
        }
        if skip_driven.is_some_and(|d| same_file(&b.file, d)) {
            skip_driven = None;
            continue;
        }
        out.push(b);
    }
    out.sort_by(|a, b| (a.condition < 30.0).cmp(&(b.condition < 30.0)).then(b.condition.total_cmp(&a.condition)));
    out.truncate(tuning().depot_buses.max(0.0) as usize);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "maps/Berlin-Spandau/global.cfg";

    fn company() -> Company {
        Company::found("Spandauer Verkehrsbetriebe", "SVB", 0, MAP, "Spandau Nord", Level::Normal, 1000)
    }

    fn run(time: u64, bus: &str) -> Run {
        Run { time, map: MAP.into(), bus: bus.into(), line: Some("136".into()), seconds: 3600.0, metres: 24_000.0, stops: 40, early: 2, late: 3, cash: 310.0, crashes: 0, hurt: 0, driver: None }
    }

    #[test]
    fn a_new_company_has_its_starting_money() {
        let c = company();
        assert_eq!(c.balance, 250_000.0);
        assert_eq!(c.day(), 1);
        assert_eq!(c.ledger.len(), 1);
        assert_eq!(Company::found("A", "A", 0, MAP, "", Level::Easy, 0).balance, 500_000.0);
    }

    #[test]
    fn buying_selling_and_the_workshop() {
        let mut c = company();
        let nr = c.buy("Vehicles/MAN_NL202/MAN_NL202.bus", "MAN NL202").unwrap();
        assert_eq!(nr, 1001);
        assert_eq!(c.balance, 250_000.0 - 160_000.0);
        assert!(c.buy("Vehicles/MAN_SD202/SD202.bus", "MAN SD202").is_err(), "not enough money left");
        c.buses[0].condition = 60.0;
        assert_eq!(c.repair_cost(nr), Some(16_000.0));
        c.repair(nr).unwrap();
        assert_eq!(c.buses[0].condition, 100.0);
        let v = c.sell(nr).unwrap();
        assert_eq!(v, 80_000.0);
        assert!(c.buses.is_empty());
        assert_eq!(c.balance, 250_000.0 - 160_000.0 - 16_000.0 + 80_000.0);
    }

    #[test]
    fn a_run_is_booked_once_with_its_takings_and_costs() {
        let mut c = company();
        let nr = c.buy("Vehicles/MAN_NL202/MAN_NL202.bus", "MAN NL202").unwrap();
        c.take_line("136");
        c.assign_bus(nr, "136");
        let before = c.balance;
        let b = c.book(&run(2000, "vehicles\\man_nl202\\MAN_NL202.bus")).unwrap();
        assert_eq!(b.bus, Some(nr));
        // 310 tickets + 35 punctual stops x 0.60
        assert!((b.income - (310.0 + 35.0 * 0.6)).abs() < 1e-9);
        // diesel: 24 km x 0.42 l x 1.50 €
        assert!((b.costs - 24.0 * 0.42 * 1.5).abs() < 1e-9);
        assert_eq!(c.buses[0].km, 24.0);
        assert!(c.buses[0].condition < 100.0);
        // the hour also paid the rent and brought the city's money for the line
        let day = 1.0 / hours_per_day();
        let expected = before + b.income - b.costs - 400.0 * day + 200.0 * day;
        assert!((c.balance - expected).abs() < 1e-6, "{} vs {}", c.balance, expected);
        assert_eq!(c.hours, 1.0);
        assert_eq!(c.trips.len(), 1);
        assert!(c.book(&run(2000, "x.bus")).is_none(), "booked twice");
        assert!(c.book(&run(500, "x.bus")).is_none(), "before the founding");
        let mut other = run(3000, "x.bus");
        other.map = "maps/Grundorf/global.cfg".into();
        assert!(c.book(&other).is_none(), "another map");
        assert!((c.punctuality() - 35.0 / 40.0).abs() < 1e-9);
    }

    #[test]
    fn the_price_of_a_run_is_what_booking_it_does() {
        let mut c = company();
        let nr = c.buy("Vehicles/MAN_NL202/MAN_NL202.bus", "MAN NL202").unwrap();
        let r = run(2000, "Vehicles/MAN_NL202/MAN_NL202.bus");
        let p = c.price(&r);
        assert_eq!(p.own_bus, Some(0));
        assert_eq!(p.rental, 0.0);
        let before = c.balance;
        let b = c.book(&r).unwrap();
        assert_eq!(b.bus, Some(nr));
        assert!((b.income - p.income()).abs() < 1e-9 && (b.costs - p.costs()).abs() < 1e-9);
        assert!((c.trips[0].result - p.result()).abs() < 0.01, "{} vs {}", c.trips[0].result, p.result());
        assert!(c.balance < before + p.result() + 1e-9, "the hour's rent comes on top");
        // a bus of nobody: rent
        assert!(c.price(&run(3000, "other.bus")).rental > 0.0);
    }

    #[test]
    fn a_friends_run_pays_no_rent_and_moves_no_time() {
        let mut c = company();
        let mut r = run(2000, "Vehicles/Their/own.bus");
        r.driver = Some("Tobi_96".into());
        let p = c.price(&r);
        assert_eq!(p.rental, 0.0);
        c.book(&r).unwrap();
        assert_eq!(c.hours, 0.0);
        assert!(c.ledger.iter().all(|e| e.kind != Kind::Rent && e.kind != Kind::Rental));
        assert!(c.trips[0].bus.ends_with("· Tobi_96"));
    }

    #[test]
    fn a_hired_bus_costs_rent_and_crashes_cost_fines() {
        let mut c = company();
        let mut r = run(2000, "Vehicles/Other/other.bus");
        r.crashes = 1;
        let b = c.book(&r).unwrap();
        let expect = 24.0 * 0.42 * 1.5 + 60.0 + 1_000.0;
        assert!((b.costs - expect).abs() < 1e-9, "{}", b.costs);
        assert_eq!(b.bus, None);
    }

    #[test]
    fn hired_drivers_earn_on_their_lines_and_cost_wages() {
        let mut c = Company::found("A", "A", 0, MAP, "", Level::Easy, 0);
        let nr = c.buy("a.bus", "MAN NL202").unwrap();
        c.take_line("136");
        c.assign_bus(nr, "136");
        let mut h = candidates(1)[2].clone();
        h.line = "136".into();
        c.hire(h.clone());
        c.run(hours_per_day() * 30.0); // a month
        let month = c.by_kind(1, c.day() + 1);
        let get = |k: Kind| month.iter().find(|(x, _)| *x == k).map(|x| x.1).unwrap_or(0.0);
        assert!((get(Kind::Wages) + h.wage).abs() < 1e-6, "{}", get(Kind::Wages));
        assert!(get(Kind::AiTakings) > 0.0);
        assert!((get(Kind::Subsidy) - 300.0 * 30.0).abs() < 1e-6);
        assert!(c.buses[0].km > 0.0 && c.buses[0].condition < 100.0);
        // without a bus on the line the driver earns nothing
        let mut d = Company::found("B", "B", 0, MAP, "", Level::Easy, 0);
        d.hire(h);
        d.run(10.0);
        assert!(d.ledger.iter().all(|e| e.kind != Kind::AiTakings));
    }

    #[test]
    fn giving_a_line_back_sends_its_buses_and_drivers_to_the_depot() {
        let mut c = company();
        let nr = c.buy("a.bus", "Bus").unwrap();
        c.take_line("137");
        c.assign_bus(nr, "137");
        c.hire(Hire { name: "X".into(), skill: 0.5, wage: 2600.0, line: "137".into() });
        c.drop_line("137");
        assert!(c.lines.is_empty());
        assert_eq!(c.buses[0].line, "");
        assert_eq!(c.drivers[0].line, "");
    }

    #[test]
    fn the_file_keeps_everything() {
        let mut c = company();
        let nr = c.buy("Vehicles/MAN_NL202/MAN_NL202.bus", "MAN NL202").unwrap();
        c.take_line("136");
        c.take_line("N34");
        c.assign_bus(nr, "136");
        c.hire(Hire { name: "Jens Weber".into(), skill: 0.8, wage: 3100.0, line: "136".into() });
        c.book(&run(2000, "Vehicles/MAN_NL202/MAN_NL202.bus")).unwrap();
        c.shared = true;
        let back = Company::from_text(&c.to_text()).unwrap();
        assert_eq!(back.name, c.name);
        assert_eq!(back.lines, c.lines);
        assert_eq!(back.seen, c.seen);
        assert_eq!(back.buses.len(), 1);
        assert_eq!(back.buses[0].file, c.buses[0].file);
        assert!((back.buses[0].condition - c.buses[0].condition).abs() < 1e-3);
        assert_eq!(back.drivers, c.drivers);
        assert_eq!(back.ledger.len(), c.ledger.len());
        assert!((back.balance - c.balance).abs() < 0.01);
        assert_eq!(back.trips.len(), 1);
        assert!(back.shared);
        assert!(Company::from_text("nothing").is_err());
        let dir = std::env::temp_dir().join(format!("omsi-company-test-{}", std::process::id()));
        let p = dir.join("company.cfg");
        c.save(&p).unwrap();
        assert_eq!(Company::load(&p).unwrap().name, c.name);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn weeks_sums_and_notices() {
        let mut c = company();
        let nr = c.buy("a.bus", "Bus").unwrap();
        c.take_line("136");
        c.assign_bus(nr, "136");
        c.run(hours_per_day() * 10.0);
        let w = c.weeks(4);
        assert_eq!(w.len(), 4);
        assert!(w[3].1 > 0.0 && w[2].1 > 0.0 && w[0] == (0.0, 0.0));
        c.buses[0].condition = 20.0;
        let n = c.notices(5);
        assert!(n.iter().any(|(s, t, v)| *s == 2 && t.contains("workshop") && v.starts_with("1001")));
        assert_eq!(money(248_350.4), "248.350 €");
        assert_eq!(money(-1200.0), "−1.200 €");
        assert_eq!(money(5.0), "5 €");
    }

    #[test]
    fn the_values_file_reads_back_and_keeps_defaults_for_what_is_missing() {
        let d = Tuning::default();
        assert_eq!(Tuning::parse(&d.to_text()), d);
        let t = Tuning::parse("[normal]\ndiesel=2,5\nstartgeld=-5\n[allgemein]\npreis_solobus=120000\nstunden_pro_tag=0\n");
        assert_eq!(t.levels[1].diesel, 2.5);
        assert_eq!(t.levels[1].start_money, d.levels[1].start_money, "a negative value is ignored");
        assert_eq!(t.price_solo, 120_000.0);
        assert_eq!(t.hours_per_day, 0.1);
        assert_eq!(t.levels[0], d.levels[0]);
        assert!(d.to_text().contains("# Dieselpreis"));
    }

    #[test]
    fn prices_by_kind_of_bus() {
        assert_eq!(price_for("MAN SD202", "Vehicles/MAN_SD202/SD202.bus"), 195_000.0);
        assert_eq!(price_for("MAN NG272", "Vehicles/MAN_NG/ng272.bus"), 240_000.0);
        assert_eq!(price_for("MAN NL202", "Vehicles/MAN_NL/nl202.bus"), 160_000.0);
        assert!(same_file("C:/OMSI 2/Vehicles/A/b.bus", "vehicles\\a\\B.bus"));
        assert!(!same_file("", "x"));
        let c = candidates(42);
        assert_eq!(c.len(), 3);
        assert!(c[2].skill > c[0].skill && c[2].wage > c[0].wage);
    }

    #[test]
    fn a_depot_is_found_by_its_name_or_by_sounding_like_one() {
        let e: Vec<String> = ["Rathaus Spandau", "Betriebshof Spandau (1)", "Betriebshof Spandau (2)", "Zoo", "Betriebshof Spandau (3)"].iter().map(|s| s.to_string()).collect();
        assert_eq!(depot_base("Betriebshof Spandau (12)"), "Betriebshof Spandau");
        assert_eq!(depot_base("Linie (S)"), "Linie (S)");
        assert_eq!(depot_names(&e), vec!["Betriebshof Spandau", "Rathaus Spandau", "Zoo"]);
        assert_eq!(depot_entries(&e, "betriebshof spandau"), vec![1, 2, 4]);
        assert_eq!(depot_entries(&e, "Zoo"), vec![3]);
        assert_eq!(depot_entries(&e, "Depot"), vec![1, 2, 4], "an unknown name: the depot-sounding entries");
        assert!(depot_entries(&["Zoo".to_string()], "Depot").is_empty());
    }

    #[test]
    fn the_depot_holds_the_buses_not_on_the_road_the_worn_ones_last() {
        let mut c = Company::found("Test", "T", 0, "maps/x/global.cfg", "Hof", Level::Easy, 1);
        c.balance = 10_000_000.0;
        for k in 0..4 {
            c.buy(&format!("Vehicles/B{k}/b.bus"), &format!("Bus {k}")).unwrap();
        }
        c.buses[0].condition = 20.0;
        c.take_line("136");
        c.assign_bus(c.buses[1].nr, "136");
        c.hire(candidates(1)[0].clone());
        c.assign_driver(0, "136");
        let nrs = |v: Vec<&Bus>| v.iter().map(|b| b.nr).collect::<Vec<_>>();
        let (n0, n1, n2, n3) = (c.buses[0].nr, c.buses[1].nr, c.buses[2].nr, c.buses[3].nr);
        assert_eq!(nrs(depot_buses(&c, true, Some("vehicles/b2/b.bus"))), vec![n3, n0]);
        assert_eq!(nrs(depot_buses(&c, false, None)), vec![n1, n2, n3, n0]);
    }
}
