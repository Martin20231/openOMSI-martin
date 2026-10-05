//! The bus company in a multiplayer session (see `omsi_launcher_lib::company`): friends
//! drive for the host's company.
//!
//! The host - whose company has `shared` on and is on this map - tells the others every ten
//! seconds whose company it is and what its account holds (`company <balance> <short>`),
//! and they show it top left. Each of them sends the host the state of their run every
//! thirty seconds and once more when their game ends (`comprun …`); the host keeps the
//! latest of each and writes it, when it is the last or when the host's own game ends, as a
//! session summary of that friend into `~/.openomsi/sessions/`, where the launcher books it
//! for the company like a run of the host's own (without rent for the friend's bus).

use crate::app::App;
use std::collections::HashMap;

/// A friend's run as the host last heard of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FriendRun {
    pub name: String,
    /// The friend's id for the run (their game's start, unix seconds).
    pub id: u64,
    pub seconds: f64,
    pub metres: f64,
    pub stops: i32,
    pub early: i32,
    pub late: i32,
    pub cash: f64,
    pub crashes: i32,
    pub hurt: i32,
    pub line: Option<String>,
    pub bus: String,
}

/// The friends' runs the host keeps until they are written, by (player id, run id).
pub(crate) type FriendRuns = HashMap<(u32, u64), FriendRun>;

/// The line shown when a shift ends: the wage, and the overtime part when there is one.
fn shift_out_text(seconds: f64) -> String {
    use omsi_launcher_lib::company::{money, shift_pay, tuning};
    let t = tuning();
    let pay = shift_pay(seconds, t.shift_minutes * 60.0, t.driver_wage, t.overtime_extra);
    if pay.wage < 0.01 {
        return omsi_ui::tr("Clocked out. Under 10 minutes does not count as work time yet.").into_owned();
    }
    if pay.overtime_hours > 0.01 {
        omsi_ui::tr("Clocked out. Wage {wage}, overtime {overtime}.")
            .replace("{wage}", &money(pay.wage))
            .replace("{overtime}", &money(pay.overtime_hours * t.driver_wage * (1.0 + t.overtime_extra)))
    } else {
        omsi_ui::tr("Clocked out. Wage {wage}.").replace("{wage}", &money(pay.wage))
    }
}

/// Seconds between the host's company announcements, and between a friend's run updates.
const ANNOUNCE_EVERY: f32 = 10.0;
const RUN_EVERY: f32 = 30.0;

/// The `comprun` command's text: run id, whether it is the last, the numbers, the line
/// (`-` for none) and the bus file (cut to fit a command).
pub(crate) fn run_command(id: u64, last: bool, r: &FriendRun) -> String {
    let line = r.line.as_deref().map(|l| l.trim().replace(' ', "_")).filter(|l| !l.is_empty()).unwrap_or_else(|| "-".into());
    let head = format!(
        "comprun {id} {} {:.0} {:.0} {} {} {} {:.2} {} {} {line} ",
        last as u8, r.seconds, r.metres, r.stops, r.early, r.late, r.cash, r.crashes, r.hurt
    );
    let room = omsi_net::MAX_CHAT.saturating_sub(head.len());
    let mut bus = r.bus.clone();
    if bus.len() > room {
        // (the file name alone, then cut: the company only compares it with its own buses)
        bus = bus.rsplit(['/', '\\']).next().unwrap_or(&bus).to_string();
        while bus.len() > room {
            bus.pop();
        }
    }
    head + &bus
}

/// A `comprun` command's arguments (everything after the verb): (run id, last, the run).
pub(crate) fn parse_run(arg: &str) -> Option<(u64, bool, FriendRun)> {
    let mut it = arg.splitn(12, ' ');
    let id = it.next()?.parse::<u64>().ok()?;
    let last = it.next()? == "1";
    let num = |s: Option<&str>| s.and_then(|x| x.parse::<f64>().ok()).filter(|v| v.is_finite() && *v >= 0.0);
    let seconds = num(it.next())?;
    let metres = num(it.next())?;
    let stops = num(it.next())? as i32;
    let early = num(it.next())? as i32;
    let late = num(it.next())? as i32;
    let cash = num(it.next())?;
    let crashes = num(it.next())? as i32;
    let hurt = num(it.next())? as i32;
    let line = it.next()?;
    let bus = it.next().unwrap_or("").trim().to_string();
    // (a friend's numbers are theirs to send: a day of driving at most, no absurd sums)
    if seconds > 86_400.0 || metres > 3_000_000.0 || cash > 100_000.0 || stops > 10_000 {
        return None;
    }
    Some((id, last, FriendRun {
        name: String::new(),
        id,
        seconds,
        metres,
        stops,
        early: early.min(stops),
        late: late.min(stops),
        cash,
        crashes,
        hurt,
        line: (line != "-").then(|| line.replace('_', " ")),
        bus,
    }))
}

/// The session summary of a friend's run, as the launcher reads the host's own.
pub(crate) fn friend_session_json(map: &str, r: &FriendRun) -> serde_json::Value {
    serde_json::json!({
        "time": r.id,
        "driver": r.name,
        "friend": r.name,
        "map": map,
        "bus": r.bus,
        "line": r.line,
        "tour": null,
        "seconds": r.seconds,
        "metres": r.metres,
        "stops": r.stops,
        "early": r.early,
        "late": r.late,
        "tickets": 0,
        "cash": r.cash,
        "crashes": r.crashes,
        "hurt": r.hurt,
        "jolts": 0,
        "driving": 0.0,
        "comfort": 0.0,
        "ticketing": 0.0,
    })
}

impl App {
    /// The host's company, if the others may drive for it.
    fn shared_company(&self) -> Option<&omsi_launcher_lib::company::Company> {
        self.company.as_ref().filter(|c| c.shared)
    }

    /// Every frame: the host announces its company, a friend sends their run.
    pub(crate) fn company_sync(&mut self, dt: f32) {
        let Some(role) = self.lan.as_ref().map(|l| l.role) else { return };
        self.company_sync_t -= dt;
        if self.company_sync_t > 0.0 {
            return;
        }
        if role == omsi_net::Role::Host {
            self.company_sync_t = ANNOUNCE_EVERY;
            let Some(c) = self.shared_company() else { return };
            let short = if c.short.trim().is_empty() { c.name.clone() } else { c.short.clone() };
            let text = format!("company {:.0} {}", c.balance, short.trim());
            let l = self.lan.as_mut().unwrap();
            let ids: Vec<u32> = l.peers().map(|p| p.pose.id).filter(|id| *id != l.my_id).collect();
            for id in ids {
                l.command(id, &text);
            }
        } else {
            self.company_sync_t = RUN_EVERY;
            self.send_company_run(false);
        }
    }

    /// A friend's game: the state of this run to the host (`last`: the game is ending).
    pub(crate) fn send_company_run(&mut self, last: bool) {
        if self.remote_company.is_none() || self.career.seconds <= 0.0 {
            return;
        }
        if self.company_run_id == 0 {
            self.company_run_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1);
        }
        let k = &self.career;
        let r = FriendRun {
            name: String::new(),
            id: self.company_run_id,
            seconds: k.seconds,
            metres: k.metres,
            stops: k.stops[0],
            early: k.stops[1],
            late: k.stops[2],
            cash: k.tickets.1,
            crashes: k.crashes[0],
            hurt: k.crashes[1],
            line: self.args.line.clone(),
            bus: self.driven_bus_file(),
        };
        let text = run_command(self.company_run_id, last, &r);
        if let Some(l) = self.lan.as_mut().filter(|l| l.role == omsi_net::Role::Client) {
            l.command(1, &text);
        }
    }

    /// F7: the boss hides the company account and sees only a driver's shift, or switches back.
    pub(crate) fn toggle_driver_view(&mut self) {
        if self.lan.as_ref().is_some_and(|l| l.role == omsi_net::Role::Client && self.remote_company.is_some()) {
            self.service_msg = Some((omsi_ui::tr("You are a driver here. The company account stays hidden").into_owned(), 5.0));
            return;
        }
        self.as_driver = !self.as_driver;
        let text = if self.as_driver {
            omsi_ui::tr("Driver view: no company account. F7 switches back to boss")
        } else {
            omsi_ui::tr("Boss view: the company account is shown. F7 switches to the driver view")
        };
        self.service_msg = Some((text.into_owned(), 6.0));
    }

    /// F6: start the shift, or end it and pay the wage.
    pub(crate) fn toggle_company_shift(&mut self) {
        if self.company.is_none() && self.remote_company.is_none() {
            self.service_msg = Some((omsi_ui::tr("No company to clock in for").into_owned(), 4.0));
            return;
        }
        if self.shift_on {
            self.clock_out_shift();
            return;
        }
        self.shift_on = true;
        self.shift_seconds = 0.0;
        self.tablet_after = false;
        self.shift_bill = None;
        self.shift_snap = Some(crate::tablet::ShiftSnap {
            metres: self.career.metres,
            stops: self.career.stops,
            crashes: self.career.crashes[0],
        });
        self.service_msg = Some((omsi_ui::tr("Clocked in. F6 clocks out at the end of the shift").into_owned(), 6.0));
    }

    /// End the shift and pay it: the host's account here, or a note to the host when this
    /// game joined theirs.
    pub(crate) fn clock_out_shift(&mut self) {
        if !self.shift_on {
            return;
        }
        self.shift_on = false;
        let seconds = self.shift_seconds.min(14.0 * 3600.0);
        self.shift_seconds = 0.0;
        self.remember_shift(seconds);
        if matches!(self.list_kind, Some(crate::game_lists::ListKind::Tablet)) {
            self.tablet_after = true;
        }
        let client = self.lan.as_ref().is_some_and(|l| l.role == omsi_net::Role::Client) && self.remote_company.is_some();
        if client {
            let text = format!("compshift {seconds:.0}");
            if let Some(l) = self.lan.as_mut() {
                l.command(1, &text);
            }
            self.service_msg = Some((shift_out_text(seconds), 8.0));
            return;
        }
        let pay = self.company.as_mut().map(|c| c.pay_player_shift(seconds));
        if let Some(c) = self.company.as_ref() {
            if let Err(e) = c.save(&omsi_launcher_lib::company_path()) {
                log::warn!("company shift: {e}");
            }
        }
        self.company_sync_t = 0.0;
        if pay.is_some() {
            self.service_msg = Some((shift_out_text(seconds), 8.0));
        }
    }

    /// Keep the shift's kilometres, stops and crashes for the settlement.
    fn remember_shift(&mut self, seconds: f64) {
        let snap = self.shift_snap.take().unwrap_or(crate::tablet::ShiftSnap {
            metres: self.career.metres,
            stops: self.career.stops,
            crashes: self.career.crashes[0],
        });
        self.shift_bill = Some(crate::tablet::bill_from(self.career.metres, self.career.stops, self.career.crashes[0], snap, seconds));
    }

    /// The host's game: a friend clocked out (`arg` is the seconds they were on the clock).
    pub(crate) fn friend_shift(&mut self, from: u32, arg: &str) {
        if self.shared_company().is_none() {
            return;
        }
        let Some(seconds) = arg.trim().parse::<f64>().ok().filter(|s| s.is_finite() && *s >= 0.0 && *s <= 14.0 * 3600.0) else {
            log::info!("company: a shift of player {from} that does not read: '{arg}'");
            return;
        };
        let name = self.lan.as_ref().and_then(|l| l.peers().find(|p| p.pose.id == from).map(|p| p.pose.name.clone())).unwrap_or_else(|| format!("Player {from}"));
        if let Some(c) = self.company.as_mut() {
            let pay = c.pay_player_shift(seconds);
            log::info!("company: {name} clocked out after {:.0} min, wage {:.2}", seconds / 60.0, pay.wage);
        }
        if let Some(c) = self.company.as_ref() {
            if let Err(e) = c.save(&omsi_launcher_lib::company_path()) {
                log::warn!("company shift: {e}");
            }
        }
        self.company_sync_t = 0.0;
    }

    /// The host's game: a friend's run came (`from` is their player id).
    pub(crate) fn friend_run(&mut self, from: u32, arg: &str) {
        if self.shared_company().is_none() {
            return;
        }
        let Some((id, last, mut r)) = parse_run(arg) else {
            log::info!("company: a run of player {from} that does not read: '{arg}'");
            return;
        };
        r.name = self
            .lan
            .as_ref()
            .and_then(|l| l.peers().find(|p| p.pose.id == from).map(|p| p.pose.name.clone()))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| format!("Player {from}"));
        // (a run id as a player's only: two friends' games may start in the same second)
        r.id = id.saturating_mul(100) + (from as u64 % 100);
        self.friend_runs.insert((from, id), r);
        if last {
            self.write_friend_runs(Some((from, id)));
        }
    }

    /// The host's game: friends' runs written as session summaries (`which`: one of them;
    /// None: all, when the host's game ends).
    pub(crate) fn write_friend_runs(&mut self, which: Option<(u32, u64)>) {
        let keys: Vec<(u32, u64)> = match which {
            Some(k) => vec![k],
            None => self.friend_runs.keys().copied().collect(),
        };
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default();
        let dir = home.join(".openomsi").join("sessions");
        for k in keys {
            let Some(r) = self.friend_runs.remove(&k) else { continue };
            if r.seconds <= 0.0 {
                continue;
            }
            let v = friend_session_json(&self.args.map, &r);
            let path = dir.join(format!("{}-friend-{}.json", r.id, k.0));
            let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap_or_default()));
            match written {
                Ok(()) => log::info!("company: {}'s run ({:.1} km, {:.2} takings) written to {}", r.name, r.metres / 1000.0, r.cash, path.display()),
                Err(e) => log::warn!("company: {}'s run: {e}", r.name),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> FriendRun {
        FriendRun { name: String::new(), id: 1791150000, seconds: 1800.0, metres: 12345.0, stops: 20, early: 1, late: 2, cash: 155.5, crashes: 1, hurt: 0, line: Some("136".into()), bus: "Vehicles/MAN_NL202/MAN_NL202.bus".into() }
    }

    #[test]
    fn a_run_goes_over_as_a_command_and_comes_back() {
        let text = run_command(1791150000, true, &run());
        assert!(text.len() <= omsi_net::MAX_CHAT);
        let (id, last, r) = parse_run(text.strip_prefix("comprun ").unwrap()).unwrap();
        assert_eq!((id, last), (1791150000, true));
        assert_eq!(r.line.as_deref(), Some("136"));
        assert_eq!(r.bus, "Vehicles/MAN_NL202/MAN_NL202.bus");
        assert_eq!((r.stops, r.early, r.late, r.crashes), (20, 1, 2, 1));
        assert!((r.cash - 155.5).abs() < 1e-9 && (r.metres - 12345.0).abs() < 1.0);
    }

    #[test]
    fn a_long_bus_path_is_cut_and_nonsense_is_refused() {
        let mut r = run();
        r.bus = format!("Vehicles/{}/bus.bus", "x".repeat(300));
        r.line = None;
        let text = run_command(1, false, &r);
        assert!(text.len() <= omsi_net::MAX_CHAT, "{}", text.len());
        let (_, _, back) = parse_run(text.strip_prefix("comprun ").unwrap()).unwrap();
        assert_eq!(back.line, None);
        assert!(parse_run("1 0 1e12 0 0 0 0 0 0 0 - x").is_none());
        assert!(parse_run("1 0 NaN 0 0 0 0 0 0 0 - x").is_none());
        assert!(parse_run("abc").is_none());
    }

    #[test]
    fn the_friends_summary_reads_as_a_session() {
        let mut r = run();
        r.name = "Tobi_96".into();
        let v = friend_session_json("maps/Berlin-Spandau/global.cfg", &r);
        let s: omsi_launcher_lib::Session = serde_json::from_value(v).unwrap();
        assert_eq!(s.friend.as_deref(), Some("Tobi_96"));
        assert_eq!(s.stops, 20);
    }
}
