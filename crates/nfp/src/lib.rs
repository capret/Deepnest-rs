//! No-Fit-Polygon engine for opennest-rs.
//!
//! This is a Rust port of Deepnest's native C++ addon (`minkowski.cc`), which
//! used `boost::polygon` to compute the No-Fit-Polygon of two parts via a
//! Minkowski-sum convolution. The original was a Node N-API `.node` addon and
//! cannot load inside a Tauri webview, so it is reimplemented here and compiled
//! to WebAssembly. `calculate_nfp` is a drop-in replacement for the addon's
//! `calculateNFP({A, B})` and is invoked synchronously from the webview.
//!
//! The C++ version scaled coordinates to integers because `boost::polygon`'s
//! polygon-set is integer-only. The `geo` crate's boolean ops work on `f64`,
//! so this port operates directly on the input coordinates — no scaling.

use geo::orient::{Direction, Orient};
use geo::{Coord, LineString, MultiPolygon, Polygon};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// A 2D point as serialized by the frontend (`{x, y}`).
#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
struct Pt {
    x: f64,
    y: f64,
}

/// A ring plus its holes. The frontend stores holes as extra properties on a
/// JS array; the shim repackages them into this explicit shape before calling.
#[derive(Deserialize)]
struct RingIn {
    points: Vec<Pt>,
    #[serde(default)]
    children: Vec<Vec<Pt>>,
}

#[derive(Deserialize)]
struct NfpInput {
    #[serde(rename = "A")]
    a: RingIn,
    #[serde(rename = "B")]
    b: RingIn,
}

/// One output polygon: an outer ring and (optionally) hole rings.
#[derive(Serialize)]
struct RingOut {
    points: Vec<Pt>,
    children: Vec<Vec<Pt>>,
}

// --- low level geometry ------------------------------------------------------

/// Drop consecutive duplicate points so degenerate (zero-length) edges do not
/// pollute the convolution.
fn dedupe(pts: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(pts.len());
    for &p in pts {
        match out.last() {
            Some(last) if (last.x - p.x).abs() < 1e-9 && (last.y - p.y).abs() < 1e-9 => {}
            _ => out.push(p),
        }
    }
    // also drop an explicit closing point (first == last)
    if out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.x - l.x).abs() < 1e-9 && (f.y - l.y).abs() < 1e-9 {
            out.pop();
        }
    }
    out
}

/// Shoelace signed area of a ring of coords.
fn signed_area(ring: &[Coord<f64>]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

/// Minkowski convolution of two segments — produces the quadrilateral swept by
/// segment `a` translated along segment `b`. Mirrors `convolve_two_segments`.
/// Returns `None` for parallel edges, whose sum is a zero-area sliver that
/// would corrupt the boolean union.
fn convolve_two_segments(a: (Pt, Pt), b: (Pt, Pt)) -> Option<Polygon<f64>> {
    let add = |p: Pt, q: Pt| Coord { x: p.x + q.x, y: p.y + q.y };
    let ring = vec![
        add(a.0, b.1),
        add(a.0, b.0),
        add(a.1, b.0),
        add(a.1, b.1),
    ];
    if signed_area(&ring).abs() < 1e-7 {
        return None;
    }
    Some(Polygon::new(LineString::new(ring), vec![]))
}

/// Convolve two closed point sequences, appending every edge-pair quad to
/// `out`. Mirrors `convolve_two_point_sequences`, but iterates edges *with*
/// wrap-around so the closing edge of each ring is included.
fn convolve_two_point_sequences(out: &mut Vec<Polygon<f64>>, a: &[Pt], b: &[Pt]) {
    let a = dedupe(a);
    let b = dedupe(b);
    if a.len() < 2 || b.len() < 2 {
        return;
    }
    for ia in 0..a.len() {
        let a_edge = (a[ia], a[(ia + 1) % a.len()]);
        for ib in 0..b.len() {
            let b_edge = (b[ib], b[(ib + 1) % b.len()]);
            // original arg order: convolve_two_segments(seg=b_edge, along=a_edge)
            if let Some(quad) = convolve_two_segments(b_edge, a_edge) {
                out.push(quad);
            }
        }
    }
}

/// A polygon-with-holes used internally for the convolution.
struct PolyWH {
    exterior: Vec<Pt>,
    holes: Vec<Vec<Pt>>,
}

impl PolyWH {
    /// Translate the whole polygon (exterior + holes) by `d`. Mirrors Boost's
    /// `convolve(polygon, point)`.
    fn translated(&self, d: Pt) -> Polygon<f64> {
        let shift = |r: &[Pt]| -> LineString<f64> {
            r.iter()
                .map(|p| Coord { x: p.x + d.x, y: p.y + d.y })
                .collect()
        };
        Polygon::new(
            shift(&self.exterior),
            self.holes.iter().map(|h| shift(h)).collect(),
        )
    }

    fn rings(&self) -> impl Iterator<Item = &Vec<Pt>> {
        std::iter::once(&self.exterior).chain(self.holes.iter())
    }
}

/// Mirrors `convolve_two_polygon_sets`: convolves every ring of `a` with every
/// ring of `b`, plus the vertex/polygon translation terms.
fn convolve_two_polygon_sets(a: &[PolyWH], b: &[PolyWH]) -> Vec<Polygon<f64>> {
    let mut out: Vec<Polygon<f64>> = Vec::new();
    for ap in a {
        for a_ring in ap.rings() {
            for bp in b {
                for b_ring in bp.rings() {
                    convolve_two_point_sequences(&mut out, a_ring, b_ring);
                }
            }
        }
        for bp in b {
            if let Some(&b0) = bp.exterior.first() {
                out.push(ap.translated(b0));
            }
            if let Some(&a0) = ap.exterior.first() {
                out.push(bp.translated(a0));
            }
        }
    }
    out
}

// --- public api --------------------------------------------------------------

fn compute(input: &NfpInput) -> Vec<RingOut> {
    // `a` = part A: outer ring with its holes (children) as interiors.
    let a = vec![PolyWH {
        exterior: input.a.points.clone(),
        holes: input.a.children.clone(),
    }];

    // `b` = part B with points negated; the JS NFP is referenced w.r.t. B's
    // first point, so remember the shift to add back at the end.
    let (xshift, yshift) = input
        .b
        .points
        .first()
        .map(|p| (p.x, p.y))
        .unwrap_or((0.0, 0.0));
    let b = vec![PolyWH {
        exterior: input
            .b
            .points
            .iter()
            .map(|p| Pt { x: -p.x, y: -p.y })
            .collect(),
        holes: vec![],
    }];

    // Convolve, then dissolve all the overlapping pieces into clean
    // polygons-with-holes (Boost did this with `polygon_set::get`).
    let pieces: Vec<Polygon<f64>> = convolve_two_polygon_sets(&a, &b)
        .into_iter()
        // geo's boolean ops treat clockwise rings as holes, so every piece
        // must present a counter-clockwise exterior before the union.
        .map(|p| p.orient(Direction::Default))
        .collect();
    if pieces.is_empty() {
        return Vec::new();
    }
    let resolved: MultiPolygon<f64> = geo::unary_union(pieces.iter());

    resolved
        .iter()
        .filter_map(|poly| ring_out(poly, xshift, yshift))
        .collect()
}

/// Convert a resolved `geo` polygon back into the frontend's NFP shape,
/// undoing the B-first-point shift and dropping the redundant closing vertex.
fn ring_out(poly: &Polygon<f64>, xshift: f64, yshift: f64) -> Option<RingOut> {
    let conv = |ls: &LineString<f64>| -> Vec<Pt> {
        let mut v: Vec<Pt> = ls
            .coords()
            .map(|c| Pt { x: c.x + xshift, y: c.y + yshift })
            .collect();
        if v.len() > 1 {
            let (f, l) = (v[0], v[v.len() - 1]);
            if (f.x - l.x).abs() < 1e-9 && (f.y - l.y).abs() < 1e-9 {
                v.pop();
            }
        }
        v
    };
    let points = conv(poly.exterior());
    if points.len() < 3 {
        return None;
    }
    Some(RingOut {
        points,
        children: poly.interiors().iter().map(conv).collect(),
    })
}

/// Compute the No-Fit-Polygon of two parts.
///
/// `input` is JSON: `{"A": {"points": [...], "children": [[...]]},
/// "B": {"points": [...]}}`. Returns JSON: an array of
/// `{"points": [{x,y}], "children": [[{x,y}]]}`. Drop-in replacement for the
/// original C++ addon's `calculateNFP({A, B})`.
#[wasm_bindgen]
pub fn calculate_nfp(input: &str) -> String {
    let parsed: NfpInput = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => return format!("{{\"error\":\"{}\"}}", e),
    };
    let result = compute(&parsed);
    serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
}

/// Batch variant: `input` is JSON `{"Alist": [ring,...], "B": ring}`; returns a
/// JSON array of NFP results, one per entry in `Alist`. Mirrors the original
/// addon's `calculateNFPBatch`.
#[wasm_bindgen]
pub fn calculate_nfp_batch(input: &str) -> String {
    #[derive(Deserialize)]
    struct BatchIn {
        #[serde(rename = "Alist")]
        alist: Vec<RingIn>,
        #[serde(rename = "B")]
        b: RingIn,
    }
    let parsed: BatchIn = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => return format!("{{\"error\":\"{}\"}}", e),
    };
    let b_points = parsed.b.points.clone();
    let b_children = parsed.b.children.clone();
    let all: Vec<Vec<RingOut>> = parsed
        .alist
        .into_iter()
        .map(|a| {
            compute(&NfpInput {
                a,
                b: RingIn {
                    points: b_points.clone(),
                    children: b_children.clone(),
                },
            })
        })
        .collect();
    serde_json::to_string(&all).unwrap_or_else(|_| "[]".to_string())
}
