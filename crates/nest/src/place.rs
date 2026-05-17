//! The placement algorithm — a Rust port of `background.js`'s `placeParts`,
//! `getOuterNfp`, `getInnerNfp`, `getFrame` and `mergedLength`.
//!
//! One call to [`place_parts`] evaluates a single GA individual: it places
//! every part onto sheets and returns a fitness score. Deepnest ran this in a
//! hidden Electron window; here it runs natively in one `run_nest` command.

use std::collections::HashMap;

use serde::Serialize;

use crate::clip::{difference, union};
use crate::geom::{
    almost_equal, get_hull, get_polygon_bounds, polygon_area, rotate_polygon, shift_polygon, Poly,
    Pt, TOL,
};
use crate::nfp::nfp;
use crate::Config;

/// Persistent No-Fit-Polygon cache (Deepnest's `window.nfpcache` / `db`).
/// Shared across `run_nest` calls so repeated part pairs are computed once.
#[derive(Default)]
pub struct NfpCache {
    outer: HashMap<String, Poly>,
    inner: HashMap<String, Vec<Poly>>,
}

fn cache_key(a_src: i64, b_src: i64, a_rot: f64, b_rot: f64) -> String {
    format!(
        "A{}B{}Arot{}Brot{}",
        a_src, b_src, a_rot as i64, b_rot as i64
    )
}

// --- output types ------------------------------------------------------------

/// One placed part — the "shiftvector" Deepnest pushed into `placements`.
#[derive(Debug, Clone, Serialize)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub id: i64,
    pub source: i64,
    pub rotation: f64,
    #[serde(rename = "mergedLength", skip_serializing_if = "Option::is_none")]
    pub merged_length: Option<f64>,
    #[serde(rename = "mergedSegments", skip_serializing_if = "Vec::is_empty")]
    pub merged_segments: Vec<[Pt; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hull: Option<Vec<Pt>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hullsheet: Option<Vec<Pt>>,
}

#[derive(Debug, Serialize)]
pub struct SheetPlacement {
    pub sheet: i64,
    pub sheetid: i64,
    pub sheetplacements: Vec<Placement>,
}

#[derive(Debug, Serialize)]
pub struct NestResult {
    pub placements: Vec<SheetPlacement>,
    pub fitness: f64,
    pub area: f64,
    #[serde(rename = "mergedLength")]
    pub merged_length: f64,
    pub index: i64,
}

// --- NFP ---------------------------------------------------------------------

/// Largest-area polygon of an NFP result — the outer NFP boundary.
fn largest(polys: Vec<Poly>) -> Option<Poly> {
    polys
        .into_iter()
        .map(|p| {
            let a = polygon_area(&p.points).abs();
            (a, p)
        })
        .reduce(|best, cur| if cur.0 > best.0 { cur } else { best })
        .map(|(_, p)| p)
}

/// `background.js` `getOuterNfp`: NFP of `b` around `a`, cached.
fn get_outer_nfp(cache: &mut NfpCache, a: &Poly, b: &Poly, inside: bool) -> Option<Poly> {
    let key = cache_key(a.source, b.source, a.rotation, b.rotation);
    if let Some(doc) = cache.outer.get(&key) {
        return Some(doc.clone());
    }
    let result = largest(nfp(a, b))?;
    if result.points.len() < 3 {
        return None;
    }
    if !inside && a.source >= 0 && b.source >= 0 {
        cache.outer.insert(key, result.clone());
    }
    Some(result)
}

/// `background.js` `getFrame`: a bounding rectangle enclosing `a`, expanded by
/// 10%, with `a` itself as its single hole.
fn get_frame(a: &Poly) -> Option<Poly> {
    let b = get_polygon_bounds(&a.points)?;
    let width = b.width * 1.1;
    let height = b.height * 1.1;
    let x = b.x - 0.5 * (width - b.width);
    let y = b.y - 0.5 * (height - b.height);
    Poly {
        points: vec![
            Pt::new(x, y),
            Pt::new(x + width, y),
            Pt::new(x + width, y + height),
            Pt::new(x, y + height),
        ],
        children: vec![a.clone()],
        rotation: 0.0,
        source: a.source,
        id: -1,
    }
    .into()
}

/// `background.js` `getInnerNfp`: the region(s) in which `b` fits inside `a`.
fn get_inner_nfp(cache: &mut NfpCache, a: &Poly, b: &Poly, _config: &Config) -> Option<Vec<Poly>> {
    let cached = a.source >= 0 && b.source >= 0;
    let key = cache_key(a.source, b.source, 0.0, b.rotation);
    if cached {
        if let Some(doc) = cache.inner.get(&key) {
            return Some(doc.clone());
        }
    }

    let frame = get_frame(a)?;
    let frame_nfp = get_outer_nfp(cache, &frame, b, true)?;
    if frame_nfp.children.is_empty() {
        return None;
    }
    // the holes of the frame-NFP are the inner-fit regions
    let inner: Vec<Poly> = frame_nfp.children.clone();

    // subtract the NFP of each hole of `a`
    let mut holes: Vec<Poly> = Vec::new();
    for child in &a.children {
        if let Some(h) = get_outer_nfp(cache, child, b, false) {
            holes.push(h);
        }
    }

    let result = if holes.is_empty() {
        inner
    } else {
        let diff = difference(&inner, &holes);
        if diff.is_empty() {
            return None;
        }
        diff
    };

    if cached {
        cache.inner.insert(key, result.clone());
    }
    Some(result)
}

// --- line merging ------------------------------------------------------------

/// `background.js` `mergedLength`: total length of edges of `p` that lie
/// collinear-and-overlapping with edges of already-placed `parts` (a shared
/// laser cut). Returns `(total_length, segments)`.
///
/// Note: the original reused the variable `min2` for two purposes via JS
/// `var` hoisting; this port keeps `min2` as the intended short-edge cutoff
/// (`minlength^2`) throughout, matching the function's documented purpose.
fn merged_length(
    parts: &[Poly],
    p: &Poly,
    minlength: f64,
    tolerance: f64,
) -> (f64, Vec<[Pt; 2]>) {
    let min2 = minlength * minlength;
    let mut total_length = 0.0;
    let mut segments: Vec<[Pt; 2]> = Vec::new();
    let pp = &p.points;

    for i in 0..pp.len() {
        let a1 = pp[i];
        let a2 = pp[(i + 1) % pp.len()];
        if !a1.exact || !a2.exact {
            continue;
        }
        let ax2 = (a2.x - a1.x) * (a2.x - a1.x);
        let ay2 = (a2.y - a1.y) * (a2.y - a1.y);
        if ax2 + ay2 < min2 {
            continue;
        }
        let angle = (a2.y - a1.y).atan2(a2.x - a1.x);
        let (s, c) = (-angle).sin_cos();
        let (s2, c2) = angle.sin_cos();
        let rel_a2 = Pt::new(a2.x - a1.x, a2.y - a1.y);
        let rot_a2x = rel_a2.x * c - rel_a2.y * s;

        for b in parts {
            let bp = &b.points;
            if bp.len() > 1 {
                for k in 0..bp.len() {
                    let b1 = bp[k];
                    let b2 = bp[(k + 1) % bp.len()];
                    if !b1.exact || !b2.exact {
                        continue;
                    }
                    let bx2 = (b2.x - b1.x) * (b2.x - b1.x);
                    let by2 = (b2.y - b1.y) * (b2.y - b1.y);
                    if bx2 + by2 < min2 {
                        continue;
                    }
                    // B relative to A1, rotated so A1->A2 is horizontal
                    let rel_b1 = Pt::new(b1.x - a1.x, b1.y - a1.y);
                    let rel_b2 = Pt::new(b2.x - a1.x, b2.y - a1.y);
                    let rot_b1 = Pt::new(rel_b1.x * c - rel_b1.y * s, rel_b1.x * s + rel_b1.y * c);
                    let rot_b2 = Pt::new(rel_b2.x * c - rel_b2.y * s, rel_b2.x * s + rel_b2.y * c);
                    if !almost_equal(rot_b1.y, 0.0, tolerance)
                        || !almost_equal(rot_b2.y, 0.0, tolerance)
                    {
                        continue;
                    }
                    let amin = 0.0_f64.min(rot_a2x);
                    let amax = 0.0_f64.max(rot_a2x);
                    let bmin = rot_b1.x.min(rot_b2.x);
                    let bmax = rot_b1.x.max(rot_b2.x);
                    if bmin >= amax || bmax <= amin {
                        continue; // not overlapping
                    }
                    let (len, rel_c1x, rel_c2x) = if almost_equal(amin, bmin, TOL)
                        && almost_equal(amax, bmax, TOL)
                    {
                        (amax - amin, amin, amax) // A is B
                    } else if amin > bmin && amax < bmax {
                        (amax - amin, amin, amax) // A inside B
                    } else if bmin > amin && bmax < amax {
                        (bmax - bmin, bmin, bmax) // B inside A
                    } else {
                        let l = (amax.min(bmax) - amin.max(bmin)).max(0.0);
                        (l, amax.min(bmax), amin.max(bmin))
                    };

                    if len * len > min2 {
                        total_length += len;
                        let c1 = Pt::new(rel_c1x * c2 + a1.x, rel_c1x * s2 + a1.y);
                        let c2p = Pt::new(rel_c2x * c2 + a1.x, rel_c2x * s2 + a1.y);
                        segments.push([c1, c2p]);
                    }
                }
            }
            if !b.children.is_empty() {
                let (child_len, child_segs) =
                    merged_length(&b.children, p, minlength, tolerance);
                total_length += child_len;
                segments.extend(child_segs);
            }
        }
    }

    (total_length, segments)
}

// --- placement ---------------------------------------------------------------

/// Collect every candidate ring (exterior + holes) of an NFP result as flat
/// point lists — the positions Deepnest iterated from ClipperLib's flat paths.
fn candidate_rings(polys: &[Poly]) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    for p in polys {
        out.push(p.points.clone());
        for c in &p.children {
            out.push(c.points.clone());
        }
    }
    out
}

/// `background.js` `placeParts`: place every part of one GA individual.
pub fn place_parts(
    sheets_in: Vec<Poly>,
    parts_in: Vec<Poly>,
    config: &Config,
    cache: &mut NfpCache,
    mut progress: impl FnMut(f64),
) -> NestResult {
    // rotate every part by its assigned rotation
    let mut parts: Vec<Poly> = parts_in
        .iter()
        .map(|p| {
            let mut r = rotate_polygon(p, p.rotation);
            r.rotation = p.rotation;
            r.source = p.source;
            r.id = p.id;
            r
        })
        .collect();

    let total_num = parts.len().max(1);
    let mut sheets = sheets_in;
    let mut all_placements: Vec<SheetPlacement> = Vec::new();
    let mut fitness = 0.0;
    let mut total_sheet_area = 0.0;
    let mut total_merged = 0.0;
    let mut sheetarea = 0.0;
    // function-scoped in the original (JS `var`); feed the post-loop fitness.
    let mut min_width = 0.0_f64;
    let mut min_area = 0.0_f64;

    let rot_step = if config.rotations > 0.0 {
        360.0 / config.rotations
    } else {
        360.0
    };
    let rot_iters = (360.0 / config.rotations.max(1.0)).max(1.0) as i64;

    while !parts.is_empty() {
        if sheets.is_empty() {
            break;
        }
        let mut placed: Vec<Poly> = Vec::new();
        let mut placements: Vec<Placement> = Vec::new();
        let mut placed_idx: Vec<usize> = Vec::new();

        let sheet = sheets.remove(0);
        sheetarea = polygon_area(&sheet.points).abs();
        total_sheet_area += sheetarea;
        fitness += sheetarea; // +1 sheet (lower fitness is better)

        let mut clip_cache: HashMap<String, (Vec<Poly>, usize)> = HashMap::new();

        for i in 0..parts.len() {
            let mut part = parts[i].clone();

            // inner NFP: try rotations until the part fits on the sheet
            let mut sheet_nfp: Option<Vec<Poly>> = None;
            for _ in 0..rot_iters {
                sheet_nfp = get_inner_nfp(cache, &sheet, &part, config);
                if sheet_nfp.is_some() {
                    break;
                }
                let mut r = rotate_polygon(&part, rot_step);
                r.rotation = part.rotation + rot_step;
                r.source = part.source;
                r.id = part.id;
                if r.rotation > 360.0 {
                    r.rotation %= 360.0;
                }
                part = r;
                parts[i] = part.clone();
            }
            let sheet_nfp = match sheet_nfp {
                Some(s) if !s.is_empty() => s,
                _ => continue, // part unplaceable on this sheet
            };

            let mut position: Option<Placement> = None;

            // first part: top-left corner of the inner NFP
            if placed.is_empty() {
                let p0 = part.points[0];
                let mut best: Option<Placement> = None;
                for region in &sheet_nfp {
                    for pt in &region.points {
                        let cand = Placement {
                            x: pt.x - p0.x,
                            y: pt.y - p0.y,
                            id: part.id,
                            source: part.source,
                            rotation: part.rotation,
                            merged_length: None,
                            merged_segments: Vec::new(),
                            hull: None,
                            hullsheet: None,
                        };
                        best = Some(match best {
                            None => cand,
                            Some(b)
                                if cand.x < b.x
                                    || (almost_equal(cand.x, b.x, TOL) && cand.y < b.y) =>
                            {
                                cand
                            }
                            Some(b) => b,
                        });
                    }
                }
                if let Some(pos) = best {
                    placements.push(pos);
                    placed.push(part.clone());
                    placed_idx.push(i);
                }
                continue;
            }

            // union the NFPs of every already-placed part against `part`
            let clipkey = format!("s:{}r:{}", part.source, part.rotation as i64);
            let mut subjects: Vec<Poly> = Vec::new();
            let mut startindex = 0usize;
            if let Some((prev_nfp, idx)) = clip_cache.get(&clipkey) {
                subjects.extend(prev_nfp.iter().cloned());
                startindex = *idx;
            }
            let mut error = false;
            for j in startindex..placed.len() {
                match get_outer_nfp(cache, &placed[j], &part, false) {
                    Some(n) => {
                        subjects.push(shift_polygon(&n, placements[j].x, placements[j].y));
                    }
                    None => {
                        error = true;
                        break;
                    }
                }
            }
            if error {
                continue;
            }
            let combined = union(&subjects);
            if !placed.is_empty() {
                clip_cache.insert(clipkey, (combined.clone(), placed.len() - 1));
            }

            // valid positions = inner NFP minus the combined collision region
            let final_nfp = difference(&sheet_nfp, &combined);
            if final_nfp.is_empty() {
                continue;
            }
            let final_rings = candidate_rings(&final_nfp);

            // evaluate every candidate position, keep the best
            min_area = 0.0;
            min_width = 0.0;
            let mut have_min = false;
            let mut min_x: Option<f64> = None;
            let mut min_y: Option<f64> = None;

            let mut allpoints: Vec<Pt> = Vec::new();
            for m in 0..placed.len() {
                for pt in &placed[m].points {
                    allpoints.push(Pt::new(pt.x + placements[m].x, pt.y + placements[m].y));
                }
            }
            let gravity_or_box =
                config.placement_type == "gravity" || config.placement_type == "box";
            let allbounds = if gravity_or_box {
                get_polygon_bounds(&allpoints)
            } else {
                None
            };
            let partbounds = if gravity_or_box {
                get_polygon_bounds(&part.points)
            } else {
                None
            };
            let hull_allpoints = if gravity_or_box {
                Vec::new()
            } else {
                get_hull(&allpoints)
            };

            let p0 = part.points[0];
            for ring in &final_rings {
                for pt in ring {
                    let sx = pt.x - p0.x;
                    let sy = pt.y - p0.y;

                    let mut area;
                    let mut cand_width = 0.0;
                    let mut hull = None;
                    let mut hullsheet = None;

                    if let (Some(ab), Some(pb)) = (allbounds, partbounds) {
                        let rb = get_polygon_bounds(&[
                            Pt::new(ab.x, ab.y),
                            Pt::new(ab.x + ab.width, ab.y),
                            Pt::new(ab.x + ab.width, ab.y + ab.height),
                            Pt::new(ab.x, ab.y + ab.height),
                            Pt::new(pb.x + sx, pb.y + sy),
                            Pt::new(pb.x + pb.width + sx, pb.y + sy),
                            Pt::new(pb.x + pb.width + sx, pb.y + pb.height + sy),
                            Pt::new(pb.x + sx, pb.y + pb.height + sy),
                        ])
                        .unwrap();
                        area = if config.placement_type == "gravity" {
                            rb.width * 2.0 + rb.height
                        } else {
                            rb.width * rb.height
                        };
                        cand_width = rb.width;
                    } else {
                        // convex hull
                        let mut localpoints = hull_allpoints.clone();
                        for pt in &part.points {
                            localpoints.push(Pt::new(pt.x + sx, pt.y + sy));
                        }
                        let h = get_hull(&localpoints);
                        area = polygon_area(&h).abs();
                        hull = Some(h);
                        hullsheet = Some(get_hull(&sheet.points));
                    }

                    let mut merged_len = None;
                    let mut merged_segs: Vec<[Pt; 2]> = Vec::new();
                    if config.merge_lines {
                        let shifted_part = shift_polygon(&part, sx, sy);
                        let shifted_placed: Vec<Poly> = placed
                            .iter()
                            .zip(&placements)
                            .map(|(pl, pm)| shift_polygon(pl, pm.x, pm.y))
                            .collect();
                        let minlength = 0.5 * config.scale;
                        let (ml, segs) = merged_length(
                            &shifted_placed,
                            &shifted_part,
                            minlength,
                            0.1 * config.curve_tolerance,
                        );
                        area -= ml * config.time_ratio;
                        merged_len = Some(ml);
                        merged_segs = segs;
                    }

                    let better = !have_min
                        || area < min_area
                        || (almost_equal(min_area, area, TOL)
                            && (min_x.is_none() || sx < min_x.unwrap()))
                        || (almost_equal(min_area, area, TOL)
                            && min_x.is_some()
                            && almost_equal(sx, min_x.unwrap(), TOL)
                            && sy < min_y.unwrap_or(f64::INFINITY));
                    if better {
                        have_min = true;
                        min_area = area;
                        min_width = cand_width;
                        if min_x.is_none() || sx < min_x.unwrap() {
                            min_x = Some(sx);
                        }
                        if min_y.is_none() || sy < min_y.unwrap() {
                            min_y = Some(sy);
                        }
                        position = Some(Placement {
                            x: sx,
                            y: sy,
                            id: part.id,
                            source: part.source,
                            rotation: part.rotation,
                            merged_length: merged_len,
                            merged_segments: merged_segs,
                            hull,
                            hullsheet,
                        });
                    }
                }
            }

            if let Some(pos) = position {
                if let Some(ml) = pos.merged_length {
                    total_merged += ml;
                }
                placements.push(pos);
                placed.push(part.clone());
                placed_idx.push(i);
            }

            // progress signal
            let placed_count: usize = placed.len()
                + all_placements
                    .iter()
                    .map(|sp| sp.sheetplacements.len())
                    .sum::<usize>();
            progress(placed_count as f64 / total_num as f64);
        }

        fitness += min_width / sheetarea.max(f64::MIN_POSITIVE) + min_area;

        // remove placed parts (highest index first)
        placed_idx.sort_unstable();
        for &idx in placed_idx.iter().rev() {
            parts.remove(idx);
        }

        if !placements.is_empty() {
            all_placements.push(SheetPlacement {
                sheet: sheet.source,
                sheetid: sheet.id,
                sheetplacements: placements,
            });
        } else {
            break; // nothing placed — give up
        }
    }

    // parts that could not be placed are heavily penalised
    for p in &parts {
        fitness += 100_000_000.0 * (polygon_area(&p.points).abs()
            / total_sheet_area.max(f64::MIN_POSITIVE));
    }
    progress(-1.0);

    NestResult {
        placements: all_placements,
        fitness,
        area: sheetarea,
        merged_length: total_merged,
        index: 0,
    }
}
