//! The bus company's depot in the game (see `omsi_launcher_lib::company`): the company's
//! buses that do not drive a line stand on its depot, on the entry points of the map that
//! bear the depot's name, the rest in rows beside them. They are placed vehicles like any
//! other: the player walks up to one (on foot: Ctrl+Shift+G) and takes its wheel with G.

use super::*;
use omsi_launcher_lib::company::{self, Company};

/// Metres between buses standing side by side, and between two rows.
const ROW_GAP: f64 = 4.6;
const ROW_DEPTH: f64 = 16.0;
/// Buses in a row beside the depot's first place.
const PER_ROW: usize = 6;

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
pub(crate) fn park_company_buses(args: &Args, c: &Company, w: &World, renderer: &Renderer, scene: &mut Scene, player_at: Option<DVec3>) -> Option<(Vec<player::Player>, String)> {
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
                out.push(q);
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
