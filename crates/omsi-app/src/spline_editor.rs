//! The spline editor: the roads, rails, pavements and wires of the map (its `[spline]`
//! records) changed inside the game, as part of the object editor (X switches between
//! objects and splines). A spline is picked where the view points, then moved, turned,
//! made longer or shorter, bent, straightened, raised and given a gradient; C continues the
//! road with a new spline of the same kind at its end, P joins its start to the spline
//! before it, Shift+P pulls the splines after it along the chain onto its end, V gives it
//! the next type of its folder, Delete takes it away and Backspace undoes all its edits.
//!
//! This module only keeps the edits: the splines of the tiles it has looked at, as the tile
//! files have them (`base`), and per tile the records changed and the ones added
//! ([`TileEdits`]). The world reads a tile through [`TileEdits::apply`], so an edited tile
//! is read again with its meshes, lanes, ground cuts and attachment rows made from the new
//! records; saving writes them with `omsi_map::tile_write`.

use glam::{DVec2, DVec3};
use omsi_geometry::SplineCurve;
use omsi_map::MapSpline;
use std::collections::{HashMap, HashSet};

/// `text` in the interface's language (the English text is the key, see `locales/app.yml`).
fn tr(text: &str) -> String {
    omsi_ui::tr(text).into_owned()
}

/// A tile by its grid coordinates.
pub type TileKey = (i32, i32);

/// What the editor changed in one tile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileEdits {
    /// The tile's own splines changed (a deleted one too: `deleted` set), by IDCode, as
    /// they are now.
    pub changed: HashMap<i64, MapSpline>,
    /// New splines, after the tile's own.
    pub added: Vec<MapSpline>,
}

impl TileEdits {
    /// A tile's spline list as the editor left it. (A tile saved this run already has the
    /// new splines: they take their place instead of coming twice.)
    pub fn apply(&self, splines: &mut Vec<MapSpline>) {
        for s in splines.iter_mut() {
            if let Some(n) = self.changed.get(&s.id).or_else(|| self.added.iter().find(|a| a.id == s.id)) {
                *s = n.clone();
            }
        }
        for a in &self.added {
            if !splines.iter().any(|s| s.id == a.id) {
                splines.push(a.clone());
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

/// Every tile's edits.
pub type Edits = HashMap<TileKey, TileEdits>;

/// What a key does to the selected spline.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Move it by metres (x east, y north, z up).
    Move(DVec3),
    /// Turn it about its start by degrees (clockwise).
    Turn(f64),
    /// Make it longer (or shorter) by metres.
    Length(f64),
    /// Bend it: the curvature (1 / radius) grows by this much, positive to the right.
    Curve(f64),
    Straight,
    /// The gradient at both ends by this many percent.
    Grade(f64),
    Delete,
    Undo,
    /// A new spline of the same type where this one ends (or starts, when its end is taken).
    Continue,
    /// The start onto the end of the spline before it.
    Attach,
    /// The splines after it, along the chain, onto its end.
    PullChain,
    /// Another spline type (a `.sli` path as the map writes it).
    SetFile(String),
}

/// What an edit did: the message and the tiles to read again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    pub msg: String,
    pub tiles: Vec<TileKey>,
}

/// The tightest radius the editor bends a spline to (m).
pub const MIN_RADIUS: f64 = 5.0;
/// The shortest spline it makes (m).
pub const MIN_LENGTH: f64 = 0.5;
/// How far P looks for a free spline end before a spline that names none (m).
pub const ATTACH_REACH: f64 = 15.0;

pub struct SplineEditor {
    tile_size: f64,
    /// The splines of each tile looked at, as the tile reads (chrono patches applied).
    base: HashMap<TileKey, Vec<MapSpline>>,
    /// The splines as they were before the editor first changed them.
    originals: HashMap<(TileKey, i64), MapSpline>,
    pub selected: Option<(TileKey, i64)>,
    candidates: Vec<(TileKey, i64)>,
    next: usize,
    /// The IDCode the next new spline gets.
    next_id: i64,
    /// What to draw over the selected spline: points along it (start green, end red).
    pub marks: Vec<(DVec3, [f32; 3])>,
}

impl SplineEditor {
    /// `id_floor`: no IDCode of the map's objects and splines is at or above it.
    pub fn new(tile_size: f64, id_floor: i64) -> SplineEditor {
        SplineEditor {
            tile_size,
            base: HashMap::new(),
            originals: HashMap::new(),
            selected: None,
            candidates: Vec::new(),
            next: 0,
            next_id: id_floor.max(1),
            marks: Vec::new(),
        }
    }

    pub fn has_base(&self, tile: TileKey) -> bool {
        self.base.contains_key(&tile)
    }

    /// The splines of `tile` as its file (with the chrono patches) has them.
    pub fn set_base(&mut self, tile: TileKey, splines: Vec<MapSpline>) {
        if let Some(m) = splines.iter().map(|s| s.id).max() {
            self.next_id = self.next_id.max(m + 1);
        }
        self.base.insert(tile, splines);
    }

    fn origin(&self, tile: TileKey) -> DVec2 {
        DVec2::new(tile.0 as f64 * self.tile_size, tile.1 as f64 * self.tile_size)
    }

    pub fn curve(&self, tile: TileKey, s: &MapSpline) -> SplineCurve {
        SplineCurve::from_map(s, self.origin(tile))
    }

    /// A world point in `tile`'s own metres (x, y, height).
    fn local(&self, tile: TileKey, p: DVec3) -> [f64; 3] {
        let o = self.origin(tile);
        [p.x - o.x, p.y - o.y, p.z]
    }

    /// Spline `id` of `tile` as it is now.
    pub fn current(&self, edits: &Edits, tile: TileKey, id: i64) -> Option<MapSpline> {
        if let Some(e) = edits.get(&tile) {
            if let Some(s) = e.changed.get(&id) {
                return Some(s.clone());
            }
            if let Some(s) = e.added.iter().find(|s| s.id == id) {
                return Some(s.clone());
            }
        }
        self.base.get(&tile)?.iter().find(|s| s.id == id).cloned()
    }

    fn is_added(edits: &Edits, tile: TileKey, id: i64) -> bool {
        edits.get(&tile).is_some_and(|e| e.added.iter().any(|s| s.id == id))
    }

    /// Every spline the editor knows, as it is now.
    pub fn all(&self, edits: &Edits) -> Vec<(TileKey, MapSpline)> {
        let mut out = Vec::new();
        for (t, list) in &self.base {
            for s in list {
                out.push((*t, self.current(edits, *t, s.id).unwrap_or_else(|| s.clone())));
            }
        }
        for (t, e) in edits {
            out.extend(e.added.iter().map(|s| (*t, s.clone())));
        }
        out
    }

    /// Spline `id`, in `near` if it has one (two joined maps may repeat IDCodes), else in
    /// any tile looked at.
    pub fn find(&self, edits: &Edits, id: i64, near: TileKey) -> Option<(TileKey, MapSpline)> {
        if id == 0 {
            return None;
        }
        if let Some(s) = self.current(edits, near, id) {
            return Some((near, s));
        }
        let mut tiles: Vec<TileKey> = self.base.keys().chain(edits.keys()).copied().collect();
        tiles.sort();
        tiles.dedup();
        tiles.into_iter().find_map(|t| self.current(edits, t, id).map(|s| (t, s)))
    }

    /// Store spline `s` of `tile` as it is now.
    fn put(&mut self, edits: &mut Edits, tile: TileKey, s: MapSpline) {
        let e = edits.entry(tile).or_default();
        if let Some(a) = e.added.iter_mut().find(|a| a.id == s.id) {
            *a = s;
            return;
        }
        let base = self.base.get(&tile).and_then(|b| b.iter().find(|b| b.id == s.id)).cloned();
        if let Some(b) = &base {
            self.originals.entry((tile, s.id)).or_insert_with(|| b.clone());
        }
        if base.as_ref() == Some(&s) {
            e.changed.remove(&s.id);
        } else {
            e.changed.insert(s.id, s);
        }
    }

    /// The splines in front of the eye along `dir`, the one nearest the ray first.
    pub fn pick(&mut self, edits: &Edits, eye: DVec3, dir: DVec3) -> Option<(TileKey, i64)> {
        let f = dir.normalize_or_zero();
        let mut scored: Vec<(f64, TileKey, i64)> = Vec::new();
        for (tile, s) in self.all(edits) {
            if s.deleted || s.file.trim().is_empty() {
                continue;
            }
            let c = self.curve(tile, &s);
            let n = ((s.length / 2.0).ceil() as usize).clamp(1, 400);
            let mut best = f64::MAX;
            for k in 0..=n {
                let d = c.point_at(s.length * k as f64 / n as f64) - eye;
                let along = d.dot(f);
                if !(0.5..400.0).contains(&along) {
                    continue;
                }
                // (anything within 3 m of the ray is under it: a road is wide)
                let off = ((d - f * along).length() - 3.0).max(0.0) / along;
                best = best.min(off + along * 0.0004);
            }
            if best < 0.06 {
                scored.push((best, tile, s.id));
            }
        }
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.candidates = scored.into_iter().map(|s| (s.1, s.2)).take(12).collect();
        self.next = 1;
        self.selected = self.candidates.first().copied();
        self.refresh_marks(edits);
        self.selected
    }

    /// The next of the last pick's splines (Tab).
    pub fn next_pick(&mut self, edits: &Edits) -> Option<(TileKey, i64)> {
        if self.candidates.is_empty() {
            return None;
        }
        self.selected = Some(self.candidates[self.next % self.candidates.len()]);
        self.next += 1;
        self.refresh_marks(edits);
        self.selected
    }

    /// Change the selected spline.
    pub fn apply(&mut self, edits: &mut Edits, op: &Op) -> Result<Applied, String> {
        let (tile, id) = self.selected.ok_or_else(|| tr("Pick a road first: point at it and press Enter"))?;
        let mut s = self.current(edits, tile, id).ok_or_else(|| tr("The road piece is gone"))?;
        let mut tiles = vec![tile];
        let mut msg = None;
        match op {
            Op::Move(d) => {
                s.pos[0] += d.x;
                s.pos[1] += d.y;
                s.pos[2] += d.z;
                self.put(edits, tile, s);
            }
            Op::Turn(t) => {
                s.heading = (s.heading + t).rem_euclid(360.0);
                self.put(edits, tile, s);
            }
            Op::Length(d) => {
                s.length = (s.length + d).max(MIN_LENGTH);
                self.put(edits, tile, s);
            }
            Op::Curve(dk) => {
                let k = if s.radius == 0.0 { 0.0 } else { 1.0 / s.radius };
                let nk = k + dk;
                s.radius = if nk.abs() < 1e-6 {
                    0.0
                } else {
                    let r = 1.0 / nk;
                    if r.abs() < MIN_RADIUS { MIN_RADIUS * r.signum() } else { r }
                };
                self.put(edits, tile, s);
            }
            Op::Straight => {
                s.radius = 0.0;
                self.put(edits, tile, s);
            }
            Op::Grade(g) => {
                s.grad_start += g;
                s.grad_end += g;
                if let Some(dh) = s.delta_h {
                    s.delta_h = Some(dh + g / 100.0 * s.length);
                }
                self.put(edits, tile, s);
            }
            Op::SetFile(f) => {
                s.file = f.clone();
                self.put(edits, tile, s);
            }
            Op::Delete => {
                if Self::is_added(edits, tile, id) {
                    tiles.extend(self.remove_added(edits, tile, id));
                    msg = Some(tr("New piece removed"));
                } else {
                    let gone = !s.deleted;
                    s.deleted = gone;
                    let s2 = s.clone();
                    self.put(edits, tile, s);
                    tiles.extend(self.relink_neighbours(edits, tile, &s2, gone));
                }
            }
            Op::Undo => {
                if Self::is_added(edits, tile, id) {
                    tiles.extend(self.remove_added(edits, tile, id));
                    msg = Some(tr("New piece removed"));
                } else {
                    if let Some(e) = edits.get_mut(&tile) {
                        e.changed.remove(&id);
                    }
                    let back = self.current(edits, tile, id).unwrap_or(s);
                    tiles.extend(self.relink_neighbours(edits, tile, &back, false));
                    msg = Some(tr("Undone: back as in the map"));
                }
            }
            Op::Continue => {
                let (n, nt) = self.continued(edits, tile, s)?;
                let _ = n;
                msg = Some(tr("New piece added - it is selected now"));
                self.selected = Some((nt, n));
            }
            Op::Attach => {
                tiles.extend(self.attach(edits, tile, s)?);
            }
            Op::PullChain => {
                let (moved, more) = self.pull_chain(edits, tile, s);
                tiles.extend(more);
                msg = Some(if moved == 0 { tr("Nothing after it to pull along") } else { format!("{moved} {}", tr("pieces pulled along")) });
            }
        }
        tiles.sort();
        tiles.dedup();
        self.refresh_marks(edits);
        let msg = match msg {
            Some(m) => m,
            None => self.describe(edits),
        };
        Ok(Applied { msg, tiles })
    }

    /// A new spline (`id` now selected) going on from `s`: at its end when nothing follows
    /// it, else before its start when nothing comes before it, else beside it.
    fn continued(&mut self, edits: &mut Edits, tile: TileKey, mut s: MapSpline) -> Result<(i64, TileKey), String> {
        if s.deleted {
            return Err(tr("This piece is deleted (Delete brings it back)"));
        }
        let c = self.curve(tile, &s);
        let mut n = s.clone();
        n.id = self.next_id;
        self.next_id += 1;
        n.deleted = false;
        n.skew_start = 0.0;
        n.skew_end = 0.0;
        if s.next_id == 0 {
            let g = c.slope_at(s.length) * 100.0;
            n.pos = self.local(tile, c.end_point());
            n.heading = c.heading_at(s.length).rem_euclid(360.0);
            n.grad_start = g;
            n.grad_end = g;
            n.delta_h = s.delta_h.map(|_| g / 100.0 * n.length);
            n.cant_start = s.cant_end;
            n.cant_end = s.cant_end;
            n.prev_id = s.id;
            n.next_id = 0;
            n.tex_offset = s.tex_offset + s.length;
            n.map_chain_offset = s.map_chain_offset.map(|_| n.tex_offset);
            s.next_id = n.id;
            self.put(edits, tile, s);
        } else if s.prev_id == 0 {
            // a straight piece that ends where this one starts
            let g = c.slope_at(0.0) * 100.0;
            let d = SplineCurve::dir(s.heading);
            let start = DVec3::new(c.start.x - d.x * n.length, c.start.y - d.y * n.length, c.start.z - g / 100.0 * n.length);
            n.pos = self.local(tile, start);
            n.heading = s.heading;
            n.radius = 0.0;
            n.grad_start = g;
            n.grad_end = g;
            n.delta_h = s.delta_h.map(|_| g / 100.0 * n.length);
            n.cant_start = s.cant_start;
            n.cant_end = s.cant_start;
            n.next_id = s.id;
            n.prev_id = 0;
            n.tex_offset = (s.tex_offset - n.length).max(0.0);
            n.map_chain_offset = s.map_chain_offset.map(|_| n.tex_offset);
            s.prev_id = n.id;
            self.put(edits, tile, s);
        } else {
            // both ends are taken: a parallel one 6 m to the right, joined to nothing
            let d = SplineCurve::dir(s.heading);
            n.pos[0] += d.y * 6.0;
            n.pos[1] -= d.x * 6.0;
            n.prev_id = 0;
            n.next_id = 0;
        }
        let id = n.id;
        edits.entry(tile).or_default().added.push(n);
        Ok((id, tile))
    }

    /// The start of `s` onto the end of the spline before it (its `prev`, or the nearest
    /// free end within [`ATTACH_REACH`], which it is then joined to).
    fn attach(&mut self, edits: &mut Edits, tile: TileKey, mut s: MapSpline) -> Result<Vec<TileKey>, String> {
        let start = self.curve(tile, &s).point_at(0.0);
        let found = if s.prev_id != 0 {
            self.find(edits, s.prev_id, tile)
        } else {
            self.all(edits)
                .into_iter()
                .filter(|(t, x)| !x.deleted && x.next_id == 0 && !(*t == tile && x.id == s.id) && x.prev_id != s.id)
                .map(|(t, x)| {
                    let d = self.curve(t, &x).end_point().distance(start);
                    (d, t, x)
                })
                .filter(|(d, _, _)| *d <= ATTACH_REACH)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, t, x)| (t, x))
        };
        let (pt, mut p) = found.ok_or_else(|| tr("No road end within 15 m to connect to"))?;
        let pc = self.curve(pt, &p);
        let mut tiles = Vec::new();
        let (at, heading, grad, cant) = if p.next_id == s.id || (p.next_id == 0 && p.prev_id != s.id) {
            let r = (pc.end_point(), pc.heading_at(p.length), pc.slope_at(p.length) * 100.0, p.cant_end);
            if p.next_id == 0 {
                p.next_id = s.id;
                self.put(edits, pt, p.clone());
                tiles.push(pt);
            }
            r
        } else if p.prev_id == s.id {
            // the two meet start to start: this one leaves the other's start backwards
            (pc.point_at(0.0), p.heading + 180.0, -pc.slope_at(0.0) * 100.0, -p.cant_start)
        } else {
            return Err(tr("The piece before already goes on somewhere else"));
        };
        s.prev_id = p.id;
        s.pos = self.local(tile, at);
        s.heading = heading.rem_euclid(360.0);
        s.grad_start = grad;
        s.cant_start = cant;
        self.put(edits, tile, s);
        Ok(tiles)
    }

    /// The splines after `s` along its chain moved onto the end of the one before each
    /// (their own length, bend and gradient kept), as far as the tiles looked at reach.
    /// Returns how many moved and their tiles.
    fn pull_chain(&mut self, edits: &mut Edits, tile: TileKey, s: MapSpline) -> (usize, Vec<TileKey>) {
        let mut cur = (tile, s);
        let mut seen: HashSet<(TileKey, i64)> = HashSet::new();
        seen.insert((cur.0, cur.1.id));
        let mut tiles = Vec::new();
        let mut moved = 0;
        while moved < 500 {
            let Some((nt, mut n)) = self.find(edits, cur.1.next_id, cur.0) else { break };
            if n.deleted || n.prev_id != cur.1.id || !seen.insert((nt, n.id)) {
                break;
            }
            let c = self.curve(cur.0, &cur.1);
            n.pos = self.local(nt, c.end_point());
            n.heading = c.heading_at(cur.1.length).rem_euclid(360.0);
            n.grad_start = c.slope_at(cur.1.length) * 100.0;
            n.cant_start = cur.1.cant_end;
            self.put(edits, nt, n.clone());
            tiles.push(nt);
            moved += 1;
            cur = (nt, n);
        }
        (moved, tiles)
    }

    /// A new spline taken away again: the spline it went on from forgets it.
    fn remove_added(&mut self, edits: &mut Edits, tile: TileKey, id: i64) -> Vec<TileKey> {
        let Some(gone) = edits.get(&tile).and_then(|e| e.added.iter().find(|s| s.id == id).cloned()) else { return Vec::new() };
        if let Some(e) = edits.get_mut(&tile) {
            e.added.retain(|s| s.id != id);
        }
        self.selected = None;
        let mut tiles = Vec::new();
        for nid in [gone.prev_id, gone.next_id] {
            if let Some((nt, mut n)) = self.find(edits, nid, tile) {
                if n.next_id == id {
                    n.next_id = 0;
                }
                if n.prev_id == id {
                    n.prev_id = 0;
                }
                self.put(edits, nt, n);
                tiles.push(nt);
            }
        }
        tiles
    }

    /// The neighbours of `s` after it was deleted (`gone`: they forget it) or brought back
    /// (they name it again where they did before the editor came).
    fn relink_neighbours(&mut self, edits: &mut Edits, tile: TileKey, s: &MapSpline, gone: bool) -> Vec<TileKey> {
        let mut tiles = Vec::new();
        for nid in [s.prev_id, s.next_id] {
            let Some((nt, mut n)) = self.find(edits, nid, tile) else { continue };
            let before = n.clone();
            if gone {
                if n.next_id == s.id {
                    n.next_id = 0;
                }
                if n.prev_id == s.id {
                    n.prev_id = 0;
                }
            } else if let Some(o) = self.originals.get(&(nt, n.id)) {
                if o.next_id == s.id && n.next_id == 0 {
                    n.next_id = s.id;
                }
                if o.prev_id == s.id && n.prev_id == 0 {
                    n.prev_id = s.id;
                }
            }
            if n != before {
                self.put(edits, nt, n);
                tiles.push(nt);
            }
        }
        tiles
    }

    /// The points drawn over the selected spline.
    pub fn refresh_marks(&mut self, edits: &Edits) {
        self.marks.clear();
        let Some((tile, id)) = self.selected else { return };
        let Some(s) = self.current(edits, tile, id) else { return };
        let c = self.curve(tile, &s);
        let n = ((s.length / 3.0).ceil() as usize).clamp(2, 120);
        let colour = if s.deleted { [0.5, 0.5, 0.5] } else { [1.0, 0.1, 0.9] };
        for k in 0..=n {
            let p = c.point_at(s.length * k as f64 / n as f64) + DVec3::Z * 0.8;
            let col = if k == 0 {
                [0.1, 1.0, 0.2]
            } else if k == n {
                [1.0, 0.2, 0.1]
            } else {
                colour
            };
            self.marks.push((p, col));
        }
    }

    /// The selected spline and what it is like now, in plain words.
    pub fn describe(&self, edits: &Edits) -> String {
        let Some((tile, id)) = self.selected else { return tr("Spline editor: point at a road and press Enter (or click)") };
        let Some(s) = self.current(edits, tile, id) else { return tr("The road piece is gone") };
        let name = s.file.rsplit(['\\', '/']).next().unwrap_or(&s.file).to_string();
        let head = format!("{} {id} ({name})", tr("Road piece"));
        if s.deleted {
            return format!("{head}: {}", tr("deleted (Delete brings it back)"));
        }
        let bend = if s.radius == 0.0 {
            tr("straight")
        } else {
            format!("{} ({:.0} m)", tr(if s.radius > 0.0 { "curve right" } else { "curve left" }), s.radius.abs())
        };
        let slope = (s.grad_start + s.grad_end) / 2.0;
        let state = if Self::is_added(edits, tile, id) {
            format!(" · {}", tr("new"))
        } else if edits.get(&tile).is_some_and(|e| e.changed.contains_key(&id)) {
            format!(" · {}", tr("changed"))
        } else {
            String::new()
        };
        format!("{head}: {:.0} m · {bend} · {} {:.1} %{state}", s.length, tr("slope"), slope)
    }
}

/// The next spline type of the folder `current` lies in, in `names`' order (the folder's
/// `.sli` files, sorted), as the map writes the path (`current`'s folder part kept).
pub fn next_type(current: &str, names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let (dir, file) = match current.rfind(['\\', '/']) {
        Some(p) => (&current[..=p], &current[p + 1..]),
        None => ("", current),
    };
    let i = names.iter().position(|n| n.eq_ignore_ascii_case(file)).map(|i| (i + 1) % names.len()).unwrap_or(0);
    Some(format!("{dir}{}", names[i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TileKey = (0, 0);

    fn spline(id: i64, prev: i64, next: i64, pos: [f64; 3], heading: f64, length: f64) -> MapSpline {
        MapSpline { file: "Splines\\road.sli".into(), id, prev_id: prev, next_id: next, pos, heading, length, map_chain_offset: Some(0.0), ..Default::default() }
    }

    /// Two joined straight splines going north: 1 (0..50 m) and 2 (50..80 m).
    fn editor() -> (SplineEditor, Edits) {
        let mut ed = SplineEditor::new(300.0, 1000);
        ed.set_base(T, vec![spline(1, 0, 2, [100.0, 0.0, 10.0], 0.0, 50.0), spline(2, 1, 0, [100.0, 50.0, 10.0], 0.0, 30.0)]);
        ed.selected = Some((T, 1));
        (ed, Edits::new())
    }

    fn close(a: DVec3, b: DVec3) -> bool {
        a.distance(b) < 1e-6
    }

    #[test]
    fn a_pick_finds_the_spline_under_the_view() {
        let (mut ed, edits) = editor();
        // from 20 m east of the first, looking west at y = 25
        let got = ed.pick(&edits, DVec3::new(120.0, 25.0, 12.0), DVec3::new(-1.0, 0.0, -0.05));
        assert_eq!(got, Some((T, 1)));
        // looking at the second
        assert_eq!(ed.pick(&edits, DVec3::new(120.0, 70.0, 12.0), DVec3::new(-1.0, 0.0, -0.05)), Some((T, 2)));
        // looking away: nothing
        assert_eq!(ed.pick(&edits, DVec3::new(120.0, 70.0, 12.0), DVec3::new(1.0, 0.0, 0.0)), None);
        assert!(!ed.marks.is_empty() || ed.selected.is_none());
    }

    #[test]
    fn moving_bending_and_undo() {
        let (mut ed, mut edits) = editor();
        ed.apply(&mut edits, &Op::Move(DVec3::new(1.0, 0.0, 0.5))).unwrap();
        ed.apply(&mut edits, &Op::Turn(10.0)).unwrap();
        let a = ed.apply(&mut edits, &Op::Length(-60.0)).unwrap();
        assert_eq!(a.tiles, vec![T]);
        let s = ed.current(&edits, T, 1).unwrap();
        assert_eq!((s.pos, s.heading, s.length), ([101.0, 0.0, 10.5], 10.0, MIN_LENGTH));
        // bent right, then back through straight to the left
        ed.apply(&mut edits, &Op::Curve(0.01)).unwrap();
        assert!((ed.current(&edits, T, 1).unwrap().radius - 100.0).abs() < 1e-9);
        ed.apply(&mut edits, &Op::Curve(-0.01)).unwrap();
        assert_eq!(ed.current(&edits, T, 1).unwrap().radius, 0.0);
        ed.apply(&mut edits, &Op::Curve(-1.0)).unwrap();
        assert_eq!(ed.current(&edits, T, 1).unwrap().radius, -MIN_RADIUS);
        assert!(ed.describe(&edits).contains("changed"));
        ed.apply(&mut edits, &Op::Undo).unwrap();
        assert_eq!(ed.current(&edits, T, 1).unwrap(), ed.base[&T][0]);
        assert!(edits[&T].changed.is_empty());
    }

    #[test]
    fn a_gradient_keeps_a_spline_h_consistent() {
        let mut ed = SplineEditor::new(300.0, 10);
        let mut s = spline(1, 0, 0, [0.0, 0.0, 0.0], 0.0, 40.0);
        s.is_h = true;
        s.delta_h = Some(0.0);
        ed.set_base(T, vec![s]);
        ed.selected = Some((T, 1));
        let mut edits = Edits::new();
        ed.apply(&mut edits, &Op::Grade(5.0)).unwrap();
        let s = ed.current(&edits, T, 1).unwrap();
        assert_eq!((s.grad_start, s.grad_end), (5.0, 5.0));
        assert!((ed.curve(T, &s).end_point().z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn continuing_a_curve_starts_where_it_ends_and_joins_them() {
        let mut ed = SplineEditor::new(300.0, 10);
        let mut s = spline(1, 0, 0, [100.0, 100.0, 5.0], 30.0, 40.0);
        s.radius = 60.0;
        s.grad_end = 2.0;
        ed.set_base(T, vec![s.clone()]);
        ed.selected = Some((T, 1));
        let mut edits = Edits::new();
        ed.apply(&mut edits, &Op::Continue).unwrap();
        let (t, nid) = ed.selected.unwrap();
        assert_eq!(t, T);
        assert!(nid >= 10);
        let first = ed.current(&edits, T, 1).unwrap();
        let n = ed.current(&edits, T, nid).unwrap();
        assert_eq!((first.next_id, n.prev_id, n.next_id), (nid, 1, 0));
        let c0 = ed.curve(T, &first);
        let c1 = ed.curve(T, &n);
        assert!(close(c0.end_point(), c1.point_at(0.0)));
        assert!((c0.heading_at(first.length) - c1.heading_at(0.0)).abs() < 1e-9);
        assert!((n.grad_start - c0.slope_at(first.length) * 100.0).abs() < 1e-9);
        assert_eq!(n.tex_offset, 40.0);
        // taken away again: the first forgets it
        ed.apply(&mut edits, &Op::Delete).unwrap();
        assert_eq!(ed.current(&edits, T, 1).unwrap().next_id, 0);
        assert!(edits[&T].added.is_empty());
        assert_eq!(ed.selected, None);
    }

    #[test]
    fn a_spline_whose_end_is_taken_is_continued_before_its_start() {
        let (mut ed, mut edits) = editor();
        ed.apply(&mut edits, &Op::Continue).unwrap();
        let (_, nid) = ed.selected.unwrap();
        let n = ed.current(&edits, T, nid).unwrap();
        assert_eq!((n.next_id, ed.current(&edits, T, 1).unwrap().prev_id), (1, nid));
        assert!(close(ed.curve(T, &n).end_point(), DVec3::new(100.0, 0.0, 10.0)));
    }

    #[test]
    fn deleting_unjoins_the_neighbours_and_bringing_back_joins_them_again() {
        let (mut ed, mut edits) = editor();
        let a = ed.apply(&mut edits, &Op::Delete).unwrap();
        assert!(a.msg.contains("deleted"));
        assert!(ed.current(&edits, T, 1).unwrap().deleted);
        assert_eq!(ed.current(&edits, T, 2).unwrap().prev_id, 0);
        ed.apply(&mut edits, &Op::Delete).unwrap();
        assert!(!ed.current(&edits, T, 1).unwrap().deleted);
        assert_eq!(ed.current(&edits, T, 2).unwrap().prev_id, 1);
        // nothing left changed
        assert!(edits[&T].is_empty(), "{:?}", edits[&T]);
    }

    #[test]
    fn attach_and_pull_the_chain() {
        let (mut ed, mut edits) = editor();
        // the second moved away and turned, then joined again with P
        ed.selected = Some((T, 2));
        ed.apply(&mut edits, &Op::Move(DVec3::new(3.0, 2.0, 1.0))).unwrap();
        ed.apply(&mut edits, &Op::Turn(20.0)).unwrap();
        ed.apply(&mut edits, &Op::Attach).unwrap();
        let s = ed.current(&edits, T, 2).unwrap();
        assert_eq!((s.pos, s.heading), ([100.0, 50.0, 10.0], 0.0));
        // the first made longer and bent: Shift+P pulls the second onto its end
        ed.selected = Some((T, 1));
        ed.apply(&mut edits, &Op::Length(10.0)).unwrap();
        ed.apply(&mut edits, &Op::Curve(0.02)).unwrap();
        let a = ed.apply(&mut edits, &Op::PullChain).unwrap();
        assert!(a.msg.starts_with("1 piece"), "{}", a.msg);
        let first = ed.current(&edits, T, 1).unwrap();
        let second = ed.current(&edits, T, 2).unwrap();
        let c0 = ed.curve(T, &first);
        assert!(close(c0.end_point(), ed.curve(T, &second).point_at(0.0)));
        assert!((second.heading - c0.heading_at(first.length).rem_euclid(360.0)).abs() < 1e-9);
    }

    #[test]
    fn attach_finds_a_free_end_nearby_and_joins_it() {
        let mut ed = SplineEditor::new(300.0, 10);
        ed.set_base(T, vec![spline(1, 0, 0, [0.0, 0.0, 0.0], 90.0, 20.0), spline(2, 0, 0, [25.0, 4.0, 1.0], 45.0, 10.0)]);
        ed.selected = Some((T, 2));
        let mut edits = Edits::new();
        let a = ed.apply(&mut edits, &Op::Attach).unwrap();
        assert_eq!(a.tiles, vec![T]);
        let (one, two) = (ed.current(&edits, T, 1).unwrap(), ed.current(&edits, T, 2).unwrap());
        assert_eq!((one.next_id, two.prev_id), (2, 1));
        assert!(close(ed.curve(T, &two).point_at(0.0), DVec3::new(20.0, 0.0, 0.0)));
        // too far for a spline with nothing near
        ed.set_base((1, 0), vec![spline(7, 0, 0, [200.0, 200.0, 0.0], 0.0, 10.0)]);
        ed.selected = Some(((1, 0), 7));
        assert!(ed.apply(&mut edits, &Op::Attach).is_err());
    }

    #[test]
    fn the_chain_crosses_tiles() {
        let mut ed = SplineEditor::new(300.0, 10);
        ed.set_base(T, vec![spline(1, 0, 2, [100.0, 280.0, 0.0], 0.0, 20.0)]);
        ed.set_base((0, 1), vec![spline(2, 1, 0, [100.0, 0.0, 0.0], 0.0, 20.0)]);
        ed.selected = Some((T, 1));
        let mut edits = Edits::new();
        ed.apply(&mut edits, &Op::Length(5.0)).unwrap();
        let a = ed.apply(&mut edits, &Op::PullChain).unwrap();
        assert_eq!(a.tiles, vec![T, (0, 1)]);
        assert_eq!(ed.current(&edits, (0, 1), 2).unwrap().pos, [100.0, 5.0, 0.0]);
    }

    #[test]
    fn edits_apply_to_a_tiles_list() {
        let (mut ed, mut edits) = editor();
        ed.apply(&mut edits, &Op::Length(5.0)).unwrap();
        ed.selected = Some((T, 2));
        ed.apply(&mut edits, &Op::Continue).unwrap();
        let mut list = ed.base[&T].clone();
        edits[&T].apply(&mut list);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].length, 55.0);
        assert_eq!(list[1].next_id, list[2].id);
        // read again from the copy saved with them: nothing twice
        let mut saved = list.clone();
        edits[&T].apply(&mut saved);
        assert_eq!(saved, list);
    }

    #[test]
    fn the_next_type_of_the_folder() {
        let names = vec!["a.sli".to_string(), "Road.sli".to_string(), "z.sli".to_string()];
        assert_eq!(next_type("Splines\\X\\road.sli", &names).as_deref(), Some("Splines\\X\\z.sli"));
        assert_eq!(next_type("Splines\\X\\z.sli", &names).as_deref(), Some("Splines\\X\\a.sli"));
        assert_eq!(next_type("gone.sli", &names).as_deref(), Some("a.sli"));
    }
}
