//! Writing a tile's splines back: the map editor's changes to the `[spline]` / `[spline_h]`
//! records of a tile file, as the game's spline editor saves them (a copy of the tile in the
//! content folder, never the original).
//!
//! The file is changed line by line and everything the editor did not touch stays as it was
//! - unknown keywords, comments, the numbers' own spelling, the line endings. A changed
//! record gets only the lines whose value changed rewritten. A deleted record goes with the
//! `[spline_terrain_align]` and `[rule]` blocks that belong to it, and because a
//! `[splineAttachement]` names its spline by its place in the tile's spline order, the rows
//! on later splines get their index lowered and the rows on the deleted one go with it. New
//! splines are appended after the last record of the file, so no existing index moves.
//!
//! Deleting and adding need a tile of version 11 or newer (the records name their
//! neighbours by IDCode); older tiles take changes to existing records only.

use crate::tile::{MapRule, MapSpline, Tile};
use std::collections::HashMap;

/// What [`save_splines`] did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SplineSave {
    pub text: String,
    pub changed: usize,
    pub deleted: usize,
    pub added: usize,
    /// Changed splines whose IDCode the file does not have (a chrono patch's own).
    pub unmatched: Vec<i64>,
}

/// One keyword block of a tile file: its keyword (lower case) and its lines `start..end`.
#[derive(Debug, Clone)]
struct Block {
    kw: String,
    start: usize,
    end: usize,
}

fn body(l: &str) -> &str {
    l.trim_end_matches(['\r', '\n'])
}

fn keyword(l: &str) -> Option<String> {
    let t = body(l).trim();
    if t.len() < 2 || !t.starts_with('[') || !t.ends_with(']') {
        return None;
    }
    let inner = &t[1..t.len() - 1];
    if inner.is_empty() || inner.contains('[') || inner.contains(']') {
        return None;
    }
    Some(inner.to_ascii_lowercase())
}

fn blocks(lines: &[&str]) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if let Some(kw) = keyword(l) {
            if let Some(b) = out.last_mut() {
                b.end = i;
            }
            out.push(Block { kw, start: i, end: lines.len() });
        }
    }
    out
}

/// Tile versions before `[version]` was written are read as the newest (as the parser does).
fn has(version: i32, min: i32) -> bool {
    version == 0 || version >= min
}

/// A number as the map editor writes it: up to six decimals, no trailing zeros.
pub fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

fn parse(l: &str) -> Option<f64> {
    let t = body(l).trim().replace(',', ".");
    t.parse::<f64>().ok()
}

/// The line of each field of a spline record, counted from its keyword line, in the file's
/// version. `None` where the record has no such line.
#[derive(Debug, Default, Clone, Copy)]
struct Fields {
    file: usize,
    prev: Option<usize>,
    next: Option<usize>,
    /// x, height, y, heading, length, radius, gradient start, gradient end
    geo: usize,
    delta_h: Option<usize>,
    cant: Option<usize>,
    skew: Option<usize>,
    tex: Option<usize>,
}

fn fields(version: i32, is_h: bool, lines: &[&str], start: usize, end: usize) -> Fields {
    let mut o = 1;
    if has(version, 9) {
        o += 1;
    }
    let file = o;
    o += 1;
    if has(version, 6) {
        o += 1;
    }
    let (prev, next) = if has(version, 11) {
        o += 2;
        (Some(o - 2), Some(o - 1))
    } else {
        o += 1;
        (None, None)
    };
    let geo = o;
    o += 8;
    let delta_h = is_h.then(|| {
        o += 1;
        o - 1
    });
    let cant = has(version, 5).then(|| {
        o += 2;
        o - 2
    });
    // the numbers that follow, as the parser counts them: three are skew and the texture
    // offset, one or two the texture offset alone
    let mut n = 0;
    while start + o + n < end {
        let w = body(lines[start + o + n]).trim();
        if w.is_empty() || keyword(w).is_some() || w.starts_with("Object Nr.") || w.eq_ignore_ascii_case("mirror") || parse(w).is_none() {
            break;
        }
        n += 1;
    }
    let (skew, tex) = match n {
        0 => (None, None),
        1 | 2 => (None, Some(o)),
        _ => (Some(o), Some(o + 2)),
    };
    Fields { file, prev, next, geo, delta_h, cant, skew, tex }
}

/// The spline's numbers as the file stores them: unscaled from a `[worldcoordinates]` map's
/// grid (`scale` = the tile's (kx, ky), see `Tile::fit_to_world_grid`).
fn stored(s: &MapSpline, scale: (f64, f64)) -> MapSpline {
    let (kx, ky) = scale;
    let k = (kx + ky) / 2.0;
    let mut o = s.clone();
    if kx != 0.0 && ky != 0.0 {
        o.pos[0] /= kx;
        o.pos[1] /= ky;
        o.length /= k;
        o.radius /= k;
    }
    o
}

/// The tile file `text` with its splines changed (`changed`, by IDCode: the whole new
/// record, `deleted` set for one to go) and `added` appended. `scale` is the tile's
/// world-grid scale ((1, 1) on a plain map).
pub fn save_splines(text: &str, changed: &HashMap<i64, MapSpline>, added: &[MapSpline], scale: (f64, f64)) -> Result<SplineSave, String> {
    let tile = Tile::parse(&omsi_cfg::CfgFile::from_str("tile.map", text));
    let version = tile.version;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let blocks = blocks(&lines);
    let spline_blocks: Vec<usize> = blocks.iter().enumerate().filter(|(_, b)| b.kw == "spline" || b.kw == "spline_h").map(|(i, _)| i).collect();
    if spline_blocks.len() != tile.splines.len() {
        return Err(format!("the tile has {} spline records but {} were read", spline_blocks.len(), tile.splines.len()));
    }
    let eol = lines.first().map(|l| &l[body(l).len()..]).filter(|e| !e.is_empty()).unwrap_or("\r\n").to_string();
    let index_of: HashMap<i64, usize> = tile.splines.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let mut report = SplineSave::default();
    // the deleted splines, by index
    let mut deleted: Vec<usize> = Vec::new();
    // per line: None keeps it, Some(text) replaces it (empty: the line goes)
    let mut replace: HashMap<usize, String> = HashMap::new();
    let mut drop_block = vec![false; blocks.len()];
    let mut ids: Vec<i64> = changed.keys().copied().collect();
    ids.sort();
    for id in ids {
        let new = &changed[&id];
        let Some(&i) = index_of.get(&id) else {
            report.unmatched.push(id);
            continue;
        };
        let bi = spline_blocks[i];
        let b = &blocks[bi];
        if new.deleted {
            if !has(version, 11) {
                return Err(format!("tile version {version}: splines can only be deleted from tiles of version 11 or newer"));
            }
            deleted.push(i);
            drop_block[bi] = true;
            // its [spline_terrain_align] and [rule] blocks
            for (k, nb) in blocks.iter().enumerate().skip(bi + 1) {
                if matches!(nb.kw.as_str(), "spline_terrain_align" | "spline_terrain_align_2" | "rule" | "kill_rule") {
                    drop_block[k] = true;
                } else {
                    break;
                }
            }
            report.deleted += 1;
            continue;
        }
        let old = &tile.splines[i];
        let new = stored(new, scale);
        let f = fields(version, b.kw == "spline_h", &lines, b.start, b.end);
        let mut set = |off: Option<usize>, was: f64, now: f64| {
            let Some(off) = off else { return false };
            let at = b.start + off;
            if at >= b.end {
                return false;
            }
            let cur = parse(lines[at]).unwrap_or(was);
            if (cur - now).abs() <= 1e-7 * (1.0 + now.abs()) {
                return false;
            }
            replace.insert(at, format!("{}{}", num(now), &lines[at][body(lines[at]).len()..]));
            true
        };
        let mut any = false;
        any |= set(f.prev, old.prev_id as f64, new.prev_id as f64);
        any |= set(f.next, old.next_id as f64, new.next_id as f64);
        let geo_old = [old.pos[0], old.pos[2], old.pos[1], old.heading, old.length, old.radius, old.grad_start, old.grad_end];
        let geo_new = [new.pos[0], new.pos[2], new.pos[1], new.heading, new.length, new.radius, new.grad_start, new.grad_end];
        for k in 0..8 {
            any |= set(Some(f.geo + k), geo_old[k], geo_new[k]);
        }
        if let (Some(_), Some(dh)) = (f.delta_h, new.delta_h) {
            any |= set(f.delta_h, old.delta_h.unwrap_or(0.0), dh);
        }
        any |= set(f.cant, old.cant_start, new.cant_start);
        any |= set(f.cant.map(|c| c + 1), old.cant_end, new.cant_end);
        any |= set(f.skew, old.skew_start, new.skew_start);
        any |= set(f.skew.map(|c| c + 1), old.skew_end, new.skew_end);
        any |= set(f.tex, old.tex_offset, new.tex_offset);
        if !new.file.trim().is_empty() && new.file.trim() != old.file.trim() {
            let at = b.start + f.file;
            replace.insert(at, format!("{}{}", new.file.trim(), &lines[at][body(lines[at]).len()..]));
            any = true;
        }
        if any {
            report.changed += 1;
        }
    }
    if !added.is_empty() && !has(version, 11) {
        return Err(format!("tile version {version}: new splines need a tile of version 11 or newer"));
    }
    // the attachment rows: on a deleted spline they go, on a later one their index drops
    deleted.sort();
    if !deleted.is_empty() {
        for (k, b) in blocks.iter().enumerate() {
            if b.kw != "splineattachement" && b.kw != "splineattachement_repeater" {
                continue;
            }
            let mut o = 1;
            if has(version, 9) {
                o += 1;
            }
            if b.kw.ends_with("repeater") {
                o += 2;
            }
            o += 1; // file
            if has(version, 6) {
                o += 1;
            }
            let at = b.start + o;
            if at >= b.end {
                continue;
            }
            let Some(si) = parse(lines[at]).map(|v| v as i64) else { continue };
            if si < 0 {
                continue;
            }
            let si = si as usize;
            if deleted.binary_search(&si).is_ok() {
                drop_block[k] = true;
                for (j, nb) in blocks.iter().enumerate().skip(k + 1) {
                    if matches!(nb.kw.as_str(), "varparent" | "rule" | "kill_rule") {
                        drop_block[j] = true;
                    } else {
                        break;
                    }
                }
                continue;
            }
            let below = deleted.iter().filter(|d| **d < si).count();
            if below > 0 {
                replace.insert(at, format!("{}{}", si - below, &lines[at][body(lines[at]).len()..]));
            }
        }
    }
    let mut dropped = vec![false; lines.len()];
    for (k, b) in blocks.iter().enumerate() {
        if drop_block[k] {
            for d in dropped.iter_mut().take(b.end).skip(b.start) {
                *d = true;
            }
        }
    }
    let mut out = String::with_capacity(text.len() + added.len() * 300);
    for (i, l) in lines.iter().enumerate() {
        if dropped[i] {
            continue;
        }
        match replace.get(&i) {
            Some(r) => out.push_str(r),
            None => out.push_str(l),
        }
    }
    if !added.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(&eol);
        }
        for s in added {
            out.push_str(&eol);
            out.push_str(&record(&stored(s, scale), version, &eol));
            report.added += 1;
        }
    }
    report.text = out;
    Ok(report)
}

/// A whole `[spline]` record (with its `[spline_terrain_align]` and `[rule]` blocks) in
/// the format of tile version `version` (11 or newer).
pub fn record(s: &MapSpline, version: i32, eol: &str) -> String {
    let mut v: Vec<String> = Vec::new();
    v.push(if s.is_h { "[spline_h]".into() } else { "[spline]".into() });
    if has(version, 9) {
        v.push("0".into());
    }
    v.push(s.file.trim().to_string());
    v.push(s.id.to_string());
    v.push(s.prev_id.to_string());
    v.push(s.next_id.to_string());
    for x in [s.pos[0], s.pos[2], s.pos[1], s.heading, s.length, s.radius, s.grad_start, s.grad_end] {
        v.push(num(x));
    }
    if s.is_h {
        v.push(num(s.delta_h.unwrap_or(0.0)));
    }
    if has(version, 5) {
        v.push(num(s.cant_start));
        v.push(num(s.cant_end));
    }
    if has(version, 14) {
        v.push(num(s.skew_start));
        v.push(num(s.skew_end));
    }
    v.push(num(s.tex_offset));
    if s.mirror && has(version, 7) {
        v.push("mirror".into());
    }
    let mut out = v.join(eol);
    out.push_str(eol);
    if s.terrain_align_flag {
        out.push_str(&format!("{eol}[spline_terrain_align]{eol}"));
    }
    if let Some(a) = s.terrain_align {
        out.push_str(&format!("{eol}[spline_terrain_align_2]{eol}{}{eol}", num(a)));
    }
    for r in &s.rules {
        out.push_str(&rule(r, eol));
    }
    out
}

fn rule(r: &MapRule, eol: &str) -> String {
    let kw = if r.kill { "[kill_rule]" } else { "[rule]" };
    format!("{eol}{kw}{eol}{}{eol}{}{eol}{}{eol}{}{eol}", r.path_index, r.kind, num(r.value), num(r.extra))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: &str = "[version]\r\n14\r\n\r\n\
[spline]\r\n0\r\nSplines\\road.sli\r\n100\r\n0\r\n101\r\n10\r\n5.5\r\n20\r\n0\r\n50\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n\r\n\
[spline_terrain_align_2]\r\n1\r\n\r\n\
[spline]\r\n0\r\nSplines\\road.sli\r\n101\r\n100\r\n102\r\n10\r\n5.5\r\n70\r\n0\r\n30.25\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n50\r\n\r\n\
[rule]\r\n0\r\nspeedlimit\r\n30\r\n0\r\n\r\n\
[spline]\r\n0\r\nSplines\\road.sli\r\n102\r\n101\r\n0\r\n10\r\n5.5\r\n100.25\r\n0\r\n20\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n80.25\r\nmirror\r\n\r\n\
[splineAttachement]\r\n0\r\nSceneryobjects\\lamp.sco\r\n500\r\n2\r\n4\r\n0\r\n0\r\n0\r\n0\r\n0\r\n25\r\n20\r\n0\r\n0\r\n\r\n\
[splineAttachement]\r\n0\r\nSceneryobjects\\lamp.sco\r\n501\r\n1\r\n4\r\n0\r\n0\r\n0\r\n0\r\n0\r\n25\r\n20\r\n0\r\n0\r\n";

    fn parse_tile(t: &str) -> Tile {
        Tile::parse(&omsi_cfg::CfgFile::from_str("tile_0_0.map", t))
    }

    #[test]
    fn nothing_changed_is_the_same_file() {
        let r = save_splines(TILE, &HashMap::new(), &[], (1.0, 1.0)).unwrap();
        assert_eq!(r.text, TILE);
        assert_eq!((r.changed, r.deleted, r.added), (0, 0, 0));
    }

    #[test]
    fn a_changed_spline_rewrites_only_its_changed_lines() {
        let t = parse_tile(TILE);
        let mut s = t.splines[1].clone();
        s.length = 31.5;
        s.radius = -120.0;
        s.pos[2] = 6.0; // height
        s.file = "Splines\\road_2.sli".into();
        let mut ch = HashMap::new();
        ch.insert(101, s.clone());
        let r = save_splines(TILE, &ch, &[], (1.0, 1.0)).unwrap();
        assert_eq!(r.changed, 1);
        let back = parse_tile(&r.text);
        assert_eq!(back.splines.len(), 3);
        assert_eq!(back.splines[1].length, 31.5);
        assert_eq!(back.splines[1].radius, -120.0);
        assert_eq!(back.splines[1].pos, [10.0, 70.0, 6.0]);
        assert_eq!(back.splines[1].file, "Splines\\road_2.sli");
        // the other records and the rule untouched, line endings kept
        assert_eq!(back.splines[0], t.splines[0]);
        assert_eq!(back.splines[2], t.splines[2]);
        assert_eq!(back.splines[1].rules, t.splines[1].rules);
        assert!(!r.text.replace("\r\n", "").contains('\n'));
        // the unchanged x keeps its spelling, the new height is written
        assert!(r.text.contains("101\r\n100\r\n102\r\n10\r\n6\r\n70\r\n"), "{}", r.text);
    }

    #[test]
    fn a_deleted_spline_takes_its_blocks_and_rows_and_later_rows_move_down() {
        let t = parse_tile(TILE);
        let mut s = t.splines[1].clone();
        s.deleted = true;
        let mut prev = t.splines[0].clone();
        prev.next_id = 0;
        let mut next = t.splines[2].clone();
        next.prev_id = 0;
        let mut ch = HashMap::new();
        ch.insert(101, s);
        ch.insert(100, prev);
        ch.insert(102, next);
        let r = save_splines(TILE, &ch, &[], (1.0, 1.0)).unwrap();
        assert_eq!((r.deleted, r.changed), (1, 2));
        let back = parse_tile(&r.text);
        assert_eq!(back.splines.len(), 2);
        assert_eq!((back.splines[0].id, back.splines[0].next_id), (100, 0));
        assert_eq!((back.splines[1].id, back.splines[1].prev_id), (102, 0));
        assert!(back.splines[1].mirror);
        // its speed limit went with it, the first spline keeps its alignment
        assert!(!r.text.contains("speedlimit"));
        assert_eq!(back.splines[0].terrain_align, Some(1.0));
        // the row on spline 1 went, the row on spline 2 is now on spline 1
        assert_eq!(back.spline_attachments.len(), 1);
        assert_eq!((back.spline_attachments[0].id, back.spline_attachments[0].spline_index), (500, 1));
    }

    #[test]
    fn a_new_spline_is_appended_and_reads_back() {
        let t = parse_tile(TILE);
        let mut n = t.splines[2].clone();
        n.id = 900;
        n.prev_id = 102;
        n.next_id = 0;
        n.pos = [10.0, 120.25, 5.5];
        n.length = 12.5;
        n.radius = 40.0;
        n.mirror = false;
        n.terrain_align_flag = true;
        n.rules = vec![MapRule { path_index: 0, kind: "speedlimit".into(), value: 50.0, extra: 0.0, kill: false }];
        let mut last = t.splines[2].clone();
        last.next_id = 900;
        let mut ch = HashMap::new();
        ch.insert(102, last);
        let r = save_splines(TILE, &ch, &[n.clone()], (1.0, 1.0)).unwrap();
        assert_eq!((r.added, r.changed), (1, 1));
        let back = parse_tile(&r.text);
        assert_eq!(back.splines.len(), 4);
        assert_eq!(back.splines[2].next_id, 900);
        let got = &back.splines[3];
        assert_eq!((got.id, got.prev_id, got.next_id), (900, 102, 0));
        assert_eq!((got.pos, got.length, got.radius), (n.pos, 12.5, 40.0));
        assert!(got.terrain_align_flag && !got.mirror);
        assert_eq!(got.rules, n.rules);
        assert_eq!(got.map_chain_offset, Some(80.25));
        // the attachment rows keep their splines
        assert_eq!(back.spline_attachments.iter().map(|a| a.spline_index).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn a_world_grid_tile_is_written_in_its_own_measure() {
        let t = parse_tile(TILE);
        let mut s = t.splines[0].clone();
        // as read on a grid scaled by 1.25 / 0.8, then moved 1 m east on the grid
        s.pos[0] = s.pos[0] * 1.25 + 1.0;
        s.pos[1] *= 0.8;
        s.length *= 1.025;
        let mut ch = HashMap::new();
        ch.insert(100, s);
        let r = save_splines(TILE, &ch, &[], (1.25, 0.8)).unwrap();
        let back = parse_tile(&r.text);
        assert!((back.splines[0].pos[0] - 10.8).abs() < 1e-9, "{:?}", back.splines[0].pos);
        assert!((back.splines[0].pos[1] - 20.0).abs() < 1e-9);
        assert!((back.splines[0].length - 50.0).abs() < 1e-9);
    }

    #[test]
    fn old_tiles_refuse_deleting_and_adding_but_take_changes() {
        let old = "[version]\r\n10\r\n\r\n[spline]\r\n0\r\nSplines\\road.sli\r\n7\r\n-1\r\n1\r\n2\r\n3\r\n0\r\n40\r\n0\r\n0\r\n0\r\n0\r\n0\r\n";
        let t = parse_tile(old);
        let mut s = t.splines[0].clone();
        s.length = 45.0;
        let mut ch = HashMap::new();
        ch.insert(7, s.clone());
        let r = save_splines(old, &ch, &[], (1.0, 1.0)).unwrap();
        assert_eq!(parse_tile(&r.text).splines[0].length, 45.0);
        s.deleted = true;
        ch.insert(7, s);
        assert!(save_splines(old, &ch, &[], (1.0, 1.0)).is_err());
    }
}
