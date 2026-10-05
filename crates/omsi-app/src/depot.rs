//! The bus company's depot in the game (see `omsi_launcher_lib::company`): the company's
//! buses that do not drive a line stand on its depot, on the entry points of the map that
//! bear the depot's name, the rest in rows beside them. They are placed vehicles like any
//! other: the player walks up to one (on foot: Ctrl+Shift+G) and takes its wheel with G.
//! A bus under 40 % has a mechanic standing in front of it. G there starts a short repair.
//! Two or three colleagues stand in the gaps between the parked buses. They only stand.

use super::*;
use omsi_launcher_lib::company::{self, Company};

/// Metres between buses standing side by side, and between two rows.
const ROW_GAP: f64 = 4.6;
const ROW_DEPTH: f64 = 16.0;
/// Buses in a row beside the depot's first place.
const PER_ROW: usize = 6;
/// Avatar keys of the mechanics. The player is 0, other players start at 1_000_000.
const MECHANIC_KEY: u32 = 2_000_000;
/// How close one has to stand to the mechanic before G starts the repair.
const MECHANIC_REACH: f64 = 2.2;
/// Avatar keys of the colleagues on the depot. Mechanics start at 2_000_000.
const COLLEAGUE_KEY: u32 = 3_000_000;

/// Places beside `anchor` (x, y, heading in degrees) for `n` more buses: rows to its right,
/// side by side and facing the same way, none within 4 m of a place in `taken`.
pub(crate) fn rows_beside(anchor: (f64, f64, f64), n: usize, taken: &[(f64, f64)]) -> Vec<(f64, f64, f64)> {
    let h = anchor.2.to_radians();
    // (as the game puts a vehicle beside the player's: right = (cos h, -sin h))
    let right = (h.cos(), -h.sin());
    let fwd = (h.sin(), h.cos());
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    let mut k = 0usize;
    while out.len() < n && k < n * 3 + PER_ROW {
        let (col, row) = ((k % PER_ROW) as f64 + 1.0, (k / PER_ROW) as f64);
        k += 1;
        let x = anchor.0 + right.0 * col * ROW_GAP - fwd.0 * row * ROW_DEPTH;
        let y = anchor.1 + right.1 * col * ROW_GAP - fwd.1 * row * ROW_DEPTH;
        let free = taken.iter().all(|t| (t.0 - x).hypot(t.1 - y) > 4.0) && out.iter().all(|p| (p.0 - x).hypot(p.1 - y) > 4.0);
        if free {
            out.push((x, y, anchor.2));
        }
    }
    out
}

/// Put the company's waiting buses on its depot. Returns them (for `App::placed`) and the
/// depot's name, or None when buses wait but the map has no depot by that name nor one that
/// sounds like it.
pub(crate) fn park_company_buses(args: &Args, c: &Company, w: &World, renderer: &Renderer, scene: &mut Scene, player_at: Option<DVec3>) -> Option<(Vec<(player::Player, u32)>, String)> {
    let buses = company::depot_buses(c, args.schedule, args.bus.as_deref());
    if buses.is_empty() {
        // (all on the road: nothing waits, and that is no missing depot)
        return Some((Vec::new(), c.depot.clone()));
    }
    let names: Vec<String> = w.global.entry_points.iter().map(|e| e.name.clone()).collect();
    let idx = company::depot_entries(&names, &c.depot);
    if idx.is_empty() {
        log::info!("company depot: no entry point named \"{}\" nor one that sounds like a depot", c.depot);
        return None;
    }
    let depot_name = company::depot_base(&names[idx[0]]).to_string();
    // (the entry points of tiles not loaded yet come from the map index)
    let _ = w.index();
    let found: Vec<(usize, DVec3, f64)> = idx.iter().filter_map(|&i| w.entry_point_place(&w.global.entry_points[i]).map(|(p, r)| (i, p, r[0]))).collect();
    // (the rows start beside the player's own place when the game starts on the depot)
    let Some(&(_, at, heading)) = found.iter().find(|f| f.0 == args.entry).or(found.first()) else { return None };
    let anchor = (at, heading);
    let mut places: Vec<(DVec3, f64)> = found.iter().map(|f| (f.1, f.2)).collect();
    places.sort_by(|a, b| (a.0 - anchor.0).truncate().length().total_cmp(&(b.0 - anchor.0).truncate().length()));
    let near_player = |p: DVec3| player_at.is_some_and(|q| (q - p).truncate().length() < 8.0);
    let mut spots: Vec<(f64, f64, f64, Option<f64>)> = places.iter().filter(|(p, _)| !near_player(*p)).map(|(p, h)| (p.x, p.y, *h, Some(p.z))).collect();
    if spots.len() < buses.len() {
        let mut taken: Vec<(f64, f64)> = places.iter().map(|(p, _)| (p.x, p.y)).collect();
        if let Some(q) = player_at {
            taken.push((q.x, q.y));
        }
        let more = rows_beside((anchor.0.x, anchor.0.y, anchor.1), buses.len() - spots.len(), &taken);
        spots.extend(more.into_iter().map(|(x, y, h)| (x, y, h, None)));
    }
    let mut out = Vec::new();
    for (b, (x, y, h, z)) in buses.iter().zip(spots) {
        let spawn = match z {
            Some(z) => format!("{x},{y},{h},{z}"),
            None => format!("{x},{y},{h}"),
        };
        let one = Args {
            bus: Some(b.file.clone()),
            spawn: Some(spawn),
            paint: None,
            plate: None,
            number: None,
            situation_vars: Vec::new(),
            situation_strvars: Vec::new(),
            situation_others: Vec::new(),
            line: None,
            tour: None,
            trip: None,
            autostart: false,
            situation_next_stop: None,
            ..args.clone()
        };
        match spawn_player(&one, w, renderer, scene) {
            Ok(Some(q)) => {
                log::info!("company depot \"{depot_name}\": bus {} ({}, {:.0} %) at ({x:.1}, {y:.1})", b.nr, b.name, b.condition);
                out.push((q, b.nr));
            }
            Ok(None) => {}
            Err(e) => log::warn!("company depot: bus {} ({}): {e:#}", b.nr, b.file),
        }
    }
    Some((out, depot_name))
}

impl crate::app::App {
    /// The file of the bus driven now, as the launcher names it (`Vehicles/...`): the one
    /// taken on the depot counts for the company, not the one the game started with.
    pub(crate) fn driven_bus_file(&self) -> String {
        self.player
            .as_ref()
            .map(|p| {
                let file = &p.vehicle.ty.def.path;
                omsi_cfg::content_roots().iter().chain(std::iter::once(&self.args.root)).find_map(|r| file.strip_prefix(r).ok()).unwrap_or(file).to_string_lossy().replace('\\', "/")
            })
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| self.args.bus.clone().unwrap_or_default())
    }

    /// Remember which company bus this vehicle is, so the mechanic can find it later.
    /// The fleet number is not written into the bus: several buses share one file.
    pub(crate) fn remember_company_bus(&mut self, uid: u64, file: &str) {
        if uid == 0 || self.depot_fleet.iter().any(|(u, _)| *u == uid) {
            return;
        }
        let Some(c) = &self.company else { return };
        let used: Vec<u32> = self.depot_fleet.iter().map(|(_, n)| *n).collect();
        if let Some(b) = c.buses.iter().find(|b| !used.contains(&b.nr) && company::same_file(&b.file, file)) {
            self.depot_fleet.push((uid, b.nr));
        }
    }

    fn ensure_depot_points(&mut self) {
        if self.depot_pts.as_ref().is_some_and(|p| !p.is_empty()) {
            return;
        }
        let Some(w) = self.world.as_ref() else { return };
        let Some(c) = self.company.as_ref() else {
            self.depot_pts = Some(Vec::new());
            return;
        };
        let names: Vec<String> = w.global.entry_points.iter().map(|e| e.name.clone()).collect();
        let idx = company::depot_entries(&names, &c.depot);
        let pts: Vec<(f64, f64)> = idx.iter().filter_map(|&i| w.entry_point_place(&w.global.entry_points[i]).map(|(p, _)| (p.x, p.y))).collect();
        if !pts.is_empty() {
            self.depot_pts = Some(pts);
        }
    }

    fn at_workshop(&self, pos: DVec3) -> bool {
        let near_entry = self.depot_pts.as_ref().is_some_and(|pts| pts.iter().any(|(x, y)| (pos.x - x).hypot(pos.y - y) < 250.0));
        near_entry
            || self.placed.iter().any(|q| {
                self.depot_fleet.iter().any(|(u, _)| *u == q.uid) && (q.vehicle.position - pos).truncate().length() < 50.0
            })
    }

    /// Fleet number, feet, facing. One mechanic per worn bus, or one whose repair is running.
    pub(crate) fn workshop_spots(&self) -> Vec<(u32, DVec3, f64)> {
        let Some(c) = &self.company else { return Vec::new() };
        let on_foot = self.on_foot.is_some();
        let jobs = self.workshop_jobs.clone();
        let fleet = self.depot_fleet.clone();
        let conds: Vec<(u32, f64)> = c.buses.iter().map(|b| (b.nr, b.condition)).collect();
        let mut out = Vec::new();
        let add = |out: &mut Vec<(u32, DVec3, f64)>, v: &omsi_sim::VehicleInstance, nr: u32, parked: bool, at_depot: bool| {
            if out.iter().any(|s| s.0 == nr) {
                return;
            }
            let bus = company::WorkshopBus {
                nr,
                x: v.position.x,
                y: v.position.y,
                z: v.position.z,
                heading: v.heading,
                front: bus_front(v),
                condition: conds.iter().find(|(n, _)| *n == nr).map(|(_, c)| *c).unwrap_or(100.0),
                parked,
                on_foot,
                stopped: v.physics.velocity_kmh().abs() < 3.0,
                at_depot,
                seconds_left: jobs.iter().find(|(n, _)| *n == nr).map(|j| j.1),
            };
            if let Some((x, y, z, face)) = company::mechanic_of(&bus) {
                out.push((nr, DVec3::new(x, y, z), face));
            }
        };
        for q in &self.placed {
            if let Some(&nr) = fleet.iter().find(|(u, _)| *u == q.uid).map(|(_, n)| n) {
                add(&mut out, &q.vehicle, nr, true, true);
            }
        }
        if let Some((nr, pos)) = self.player.as_ref().and_then(|p| fleet.iter().find(|(u, _)| *u == p.uid).map(|(_, n)| (*n, p.vehicle.position))) {
            let at = self.at_workshop(pos);
            if let Some(p) = &self.player {
                add(&mut out, &p.vehicle, nr, false, at);
            }
        }
        out
    }

    /// Where the colleagues stand, among the company buses waiting on the depot.
    pub(crate) fn colleague_spots(&self) -> Vec<(DVec3, f64)> {
        let fleet: Vec<u64> = self.depot_fleet.iter().map(|(u, _)| *u).collect();
        let buses: Vec<company::YardSpot> = self
            .placed
            .iter()
            .filter(|q| fleet.contains(&q.uid))
            .map(|q| company::YardSpot { x: q.vehicle.position.x, y: q.vehicle.position.y, z: q.vehicle.position.z, heading: q.vehicle.heading })
            .collect();
        company::colleagues(&buses).into_iter().map(|(x, y, z, h)| (DVec3::new(x, y, z), h)).collect()
    }

    pub(crate) fn workshop_frame(&mut self, dt: f32) {
        self.ensure_depot_points();
        let mut done = Vec::new();
        for job in &mut self.workshop_jobs {
            job.1 -= dt;
            if job.1 <= 0.0 {
                done.push(job.0);
            }
        }
        self.workshop_jobs.retain(|j| j.1 > 0.0);
        for nr in done {
            self.finish_workshop(nr);
        }
        self.workshop_hint_frame();
    }

    fn finish_workshop(&mut self, nr: u32) {
        let result = self.company.as_mut().map(|c| c.repair(nr));
        match result {
            Some(Ok(cost)) => {
                if let Some(c) = self.company.as_ref() {
                    if let Err(e) = c.save(&omsi_launcher_lib::company_path()) {
                        log::warn!("workshop: {e}");
                    }
                }
                let text = omsi_ui::tr("Wagon {n} is repaired. The workshop costs {cost}.").replace("{n}", &nr.to_string()).replace("{cost}", &company::money(cost));
                self.service_msg = Some((text, 8.0));
            }
            Some(Err(e)) => self.service_msg = Some((omsi_ui::tr(&e).into_owned(), 6.0)),
            None => {}
        }
    }

    fn workshop_hint_frame(&mut self) {
        let outside = self.on_foot.as_ref().is_some_and(|f| f.inside.is_none() && f.seat.is_none());
        if outside {
            let pos = self.on_foot.as_ref().unwrap().pos.truncate();
            let near = self.workshop_spots().into_iter().filter(|s| (s.1.truncate() - pos).length() < 4.0).min_by(|a, b| (a.1.truncate() - pos).length().total_cmp(&(b.1.truncate() - pos).length()));
            match near {
                Some((nr, _, _)) if self.workshop_hint != Some(nr) && self.service_msg.is_none() => {
                    self.workshop_hint = Some(nr);
                    let left = self.workshop_jobs.iter().find(|(n, _)| *n == nr).map(|j| j.1);
                    let text = match left {
                        Some(s) => omsi_ui::tr("Wagon {n} is still in the workshop. {time} s left.").replace("{n}", &nr.to_string()).replace("{time}", &secs(s)),
                        None => {
                            let cond = self.company.as_ref().and_then(|c| c.buses.iter().find(|b| b.nr == nr).map(|b| b.condition)).unwrap_or(0.0);
                            omsi_ui::tr("The mechanic is at wagon {n}. Press G to start the repair ({time} s).")
                                .replace("{n}", &nr.to_string())
                                .replace("{time}", &secs(Company::repair_seconds(cond)))
                        }
                    };
                    self.service_msg = Some((text, 6.0));
                }
                Some(_) => {}
                None => self.workshop_hint = None,
            }
            return;
        }
        let Some((speed, pos)) = self.player.as_ref().map(|p| (p.vehicle.physics.velocity_kmh(), p.vehicle.position)) else {
            return;
        };
        let stopped = speed.abs() < 3.0;
        let here = self.at_workshop(pos);
        if !stopped || !here {
            self.workshop_told = false;
            return;
        }
        if self.workshop_told || self.service_msg.is_some() {
            return;
        }
        self.workshop_told = true;
        let spots = self.workshop_spots();
        let text = if let Some((nr, _, _)) = spots.first().copied() {
            let pct = self.company.as_ref().and_then(|c| c.buses.iter().find(|b| b.nr == nr).map(|b| b.condition.round() as i32)).unwrap_or(0);
            omsi_ui::tr("Wagon {n} needs the workshop ({pct} %). The mechanic stands at the front. Get out with Ctrl+Shift+G, then press G next to him.")
                .replace("{n}", &nr.to_string())
                .replace("{pct}", &pct.to_string())
        } else if let Some((nr, pct)) = self.player_bus_condition() {
            omsi_ui::tr("Wagon {n} is at {pct} %. The workshop starts under 40 %.").replace("{n}", &nr.to_string()).replace("{pct}", &pct.to_string())
        } else {
            return;
        };
        self.service_msg = Some((text, 8.0));
    }

    fn player_bus_condition(&self) -> Option<(u32, i32)> {
        let uid = self.player.as_ref()?.uid;
        let nr = self.depot_fleet.iter().find(|(u, _)| *u == uid).map(|(_, n)| *n)?;
        let pct = self.company.as_ref()?.buses.iter().find(|b| b.nr == nr).map(|b| b.condition.round() as i32)?;
        Some((nr, pct))
    }

    /// G beside a mechanic. True when the key belongs to the workshop, so it does not
    /// also open the driver's door.
    pub(crate) fn try_workshop(&mut self) -> bool {
        let Some(f) = self.on_foot.as_ref() else { return false };
        if f.inside.is_some() || f.seat.is_some() {
            return false;
        }
        let pos = f.pos.truncate();
        let Some((nr, _, _)) = self
            .workshop_spots()
            .into_iter()
            .filter(|s| (s.1.truncate() - pos).length() < MECHANIC_REACH)
            .min_by(|a, b| (a.1.truncate() - pos).length().total_cmp(&(b.1.truncate() - pos).length()))
        else {
            return false;
        };
        if let Some(left) = self.workshop_jobs.iter().find(|(n, _)| *n == nr).map(|j| j.1) {
            let text = omsi_ui::tr("Wagon {n} is still in the workshop. {time} s left.").replace("{n}", &nr.to_string()).replace("{time}", &secs(left));
            self.service_msg = Some((text, 4.0));
            return true;
        }
        let (cost, balance, cond) = self
            .company
            .as_ref()
            .map(|c| (c.repair_cost(nr).unwrap_or(0.0), c.balance, c.buses.iter().find(|b| b.nr == nr).map(|b| b.condition).unwrap_or(0.0)))
            .unwrap_or((0.0, 0.0, 0.0));
        if cost > 0.0 && balance < cost {
            self.service_msg = Some((omsi_ui::tr("Not enough money for the workshop").into_owned(), 5.0));
            return true;
        }
        let time = Company::repair_seconds(cond);
        self.workshop_jobs.push((nr, time));
        let text = omsi_ui::tr("Repair of wagon {n} started. About {time} s.").replace("{n}", &nr.to_string()).replace("{time}", &secs(time));
        self.service_msg = Some((text, 5.0));
        true
    }
}

fn secs(s: f32) -> String {
    format!("{}", s.round().max(1.0) as i32)
}

/// Metres from the bus origin to a spot just past the nose.
fn bus_front(v: &omsi_sim::VehicleInstance) -> f64 {
    v.ty.def.bounding_box.map(|b| (b[1] as f64 * 0.5 + b[4] as f64).abs() + 1.8).filter(|d| d.is_finite()).unwrap_or(8.5).clamp(5.0, 16.0)
}

/// Put a mechanic in front of every bus that needs one, and take away the others.
pub(crate) fn place_mechanics(h: &mut crate::humans::Humans, w: &World, r: &Renderer, scene: &mut Scene, spots: &[(u32, DVec3, f64)], shown: &mut Vec<u32>) {
    let keep: Vec<u32> = spots.iter().map(|s| s.0).collect();
    for nr in shown.iter().copied().filter(|n| !keep.contains(n)) {
        h.avatar_remove(MECHANIC_KEY.saturating_add(nr));
    }
    for &(nr, pos, heading) in spots {
        let cmd = crate::humans::AvatarCmd { pos, heading, vel: glam::DVec2::ZERO, lift: 0.0, seat: None, floor: None, aboard: None };
        h.avatar(MECHANIC_KEY.saturating_add(nr), w, r, scene, cmd, 0);
    }
    *shown = keep;
}

/// Put the colleagues where `spots` says, and take away any who are no longer needed.
pub(crate) fn place_colleagues(h: &mut crate::humans::Humans, w: &World, r: &Renderer, scene: &mut Scene, spots: &[(DVec3, f64)], shown: &mut usize) {
    for i in spots.len()..*shown {
        h.avatar_remove(COLLEAGUE_KEY + i as u32);
    }
    for (i, &(pos, heading)) in spots.iter().enumerate() {
        let cmd = crate::humans::AvatarCmd { pos, heading, vel: glam::DVec2::ZERO, lift: 0.0, seat: None, floor: None, aboard: None };
        h.avatar(COLLEAGUE_KEY + i as u32, w, r, scene, cmd, (i as u64) + 1);
    }
    *shown = spots.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stand_beside_the_depot_and_keep_clear_of_what_stands_there() {
        // heading 0: right is +x, forward +y
        let r = rows_beside((100.0, 200.0, 0.0), 8, &[(100.0 + ROW_GAP, 200.0)]);
        assert_eq!(r.len(), 8);
        assert!(r.iter().all(|p| (p.0 - (100.0 + ROW_GAP)).hypot(p.1 - 200.0) > 4.0), "the taken place stays free");
        assert!((r[0].0 - (100.0 + 2.0 * ROW_GAP)).abs() < 1e-9 && (r[0].1 - 200.0).abs() < 1e-9);
        // the second row lies behind the first
        assert!(r.iter().any(|p| (p.1 - (200.0 - ROW_DEPTH)).abs() < 1e-9));
        for (i, a) in r.iter().enumerate() {
            for b in &r[i + 1..] {
                assert!((a.0 - b.0).hypot(a.1 - b.1) > 4.0);
            }
        }
    }
}
