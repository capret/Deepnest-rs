//! Shared contour cleanup, hole detection, and millimeter normalization.
use crate::model::Part;
use nest::{
    geom::{get_polygon_bounds, polygon_area},
    Pt,
};
use std::collections::HashMap;

pub struct Import {
    pub parts: Vec<Part>,
    pub skipped: usize,
    pub assumed_mm: bool,
}

pub struct Contour {
    pub name: String,
    pub points: Vec<Pt>,
}

/// Connect only unambiguous endpoints within the chosen physical tolerance.
pub fn stitch(
    open: Vec<Contour>,
    contours: &mut Vec<Contour>,
    skipped: &mut usize,
    tolerance: f64,
) {
    let distance = |a: Pt, b: Pt| (a.x - b.x).hypot(a.y - b.y);
    let key = |p: Pt| {
        (
            (p.x / tolerance).round() as i64,
            (p.y / tolerance).round() as i64,
        )
    };
    let mut ends: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
    for (i, chain) in open.iter().enumerate() {
        if chain.points.len() < 2 {
            continue;
        }
        ends.entry(key(chain.points[0]))
            .or_default()
            .push((i, false));
        ends.entry(key(*chain.points.last().unwrap()))
            .or_default()
            .push((i, true));
    }
    let mut used = vec![false; open.len()];
    for i in 0..open.len() {
        if used[i] || open[i].points.len() < 2 {
            continue;
        }
        used[i] = true;
        let mut points = open[i].points.clone();
        while distance(points[0], *points.last().unwrap()) > tolerance {
            let end = *points.last().unwrap();
            let (x, y) = key(end);
            let mut matches = vec![];
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(items) = ends.get(&(x + dx, y + dy)) {
                        for &(j, reverse) in items {
                            let candidate = if reverse {
                                *open[j].points.last().unwrap()
                            } else {
                                open[j].points[0]
                            };
                            if !used[j] && distance(end, candidate) <= tolerance {
                                matches.push((j, reverse));
                            }
                        }
                    }
                }
            }
            if matches.len() != 1 {
                break;
            }
            let (j, reverse) = matches[0];
            used[j] = true;
            if reverse {
                points.extend(open[j].points.iter().rev().skip(1).copied());
            } else {
                points.extend(open[j].points.iter().skip(1).copied());
            }
            if points.len() > 20_000 {
                break;
            }
        }
        let closed = distance(points[0], *points.last().unwrap()) <= tolerance;
        if closed {
            points.pop();
        }
        finish(&mut points, closed, &open[i].name, contours, skipped);
    }
}

pub fn assemble(
    mut contours: Vec<Contour>,
    filename: &str,
    skipped: usize,
) -> Result<Import, String> {
    if contours.is_empty() {
        return Err("No closed cutting contours found. Close paths and convert unsupported objects to polylines first.".into());
    }
    // Sort by enclosing area so an island within a hole becomes a new part.
    contours.sort_by(|a, b| {
        polygon_area(&b.points)
            .abs()
            .total_cmp(&polygon_area(&a.points).abs())
    });
    let parents: Vec<Option<usize>> = contours
        .iter()
        .enumerate()
        .map(|(i, ring)| {
            (0..i).rev().find(|&j| {
                ring.points
                    .iter()
                    .all(|p| contains(&contours[j].points, *p))
            })
        })
        .collect();
    let mut depths = vec![0; contours.len()];
    let mut owner = vec![0; contours.len()];
    let mut parts: Vec<Part> = vec![];
    for i in 0..contours.len() {
        depths[i] = parents[i].map_or(0, |parent| depths[parent] + 1);
        if depths[i] % 2 == 0 {
            owner[i] = parts.len();
            let name = if contours[i].name.is_empty() {
                format!(
                    "{} · {}",
                    std::path::Path::new(filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(filename),
                    parts.len() + 1
                )
            } else {
                contours[i].name.clone()
            };
            parts.push(Part {
                name,
                points: contours[i].points.clone(),
                holes: vec![],
                quantity: 1,
            });
        } else {
            let parent = parents[i].unwrap();
            owner[i] = owner[parent];
            parts[owner[i]].holes.push(contours[i].points.clone());
        }
    }
    for part in &mut parts {
        let bounds = get_polygon_bounds(&part.points).unwrap();
        for p in part
            .points
            .iter_mut()
            .chain(part.holes.iter_mut().flatten())
        {
            p.x -= bounds.x;
            p.y -= bounds.y;
        }
    }
    Ok(Import {
        parts,
        skipped,
        assumed_mm: false,
    })
}

pub fn finish(
    points: &mut Vec<Pt>,
    closed: bool,
    name: &str,
    contours: &mut Vec<Contour>,
    skipped: &mut usize,
) {
    if points.is_empty() {
        return;
    }
    points.dedup_by(|a, b| (a.x - b.x).abs() < 1e-8 && (a.y - b.y).abs() < 1e-8);
    let repeats = points.len() > 2 && {
        let (first, last) = (points[0], points[points.len() - 1]);
        (first.x - last.x).hypot(first.y - last.y) < 1e-5
    };
    if repeats {
        points[0].exact |= points.last().unwrap().exact;
        points.pop();
    }
    if (closed || repeats) && points.len() >= 3 && polygon_area(points).abs() > 1e-6 {
        contours.push(Contour {
            name: name.to_string(),
            points: std::mem::take(points),
        });
    } else {
        *skipped += 1;
        points.clear();
    }
}

pub fn contains(ring: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    for (a, b) in ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
    {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
