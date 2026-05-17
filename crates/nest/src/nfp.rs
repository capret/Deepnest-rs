//! No-Fit-Polygon via Minkowski-sum convolution.
//!
//! Rust port of Deepnest's C++ addon (`minkowski.cc`, Boost.Polygon). The
//! original scaled to integers because Boost's polygon-set is integer-only;
//! `geo`'s boolean ops work on `f64`, so this runs at native scale.
//!
//! `nfp(a, b)` is the No-Fit-Polygon of part `b` around part `a`: the locus of
//! `b`'s reference point for which `b` touches but does not overlap `a`. Holes
//! of `a` (`a.children`) are honoured; holes of `b` are ignored, exactly as
//! `minkowski.cc` did.

use crate::geom::{Poly, Pt};
use geo::orient::{Direction, Orient};
use geo::{Coord, LineString, MultiPolygon, Polygon};

/// Shoelace signed area of a coord ring.
fn signed_area(r: &[Coord<f64>]) -> f64 {
    let n = r.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        let p = r[i];
        let q = r[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

/// Drop consecutive duplicate points and any explicit closing vertex.
fn dedupe(pts: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(pts.len());
    for &p in pts {
        match out.last() {
            Some(l) if (l.x - p.x).abs() < 1e-9 && (l.y - p.y).abs() < 1e-9 => {}
            _ => out.push(p),
        }
    }
    if out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.x - l.x).abs() < 1e-9 && (f.y - l.y).abs() < 1e-9 {
            out.pop();
        }
    }
    out
}

/// Minkowski sum of two segments — the swept quadrilateral. `None` for
/// parallel edges (a zero-area sliver that would corrupt the union).
fn convolve_two_segments(a: (Pt, Pt), b: (Pt, Pt)) -> Option<Polygon<f64>> {
    let add = |p: Pt, q: Pt| Coord { x: p.x + q.x, y: p.y + q.y };
    let r = vec![
        add(a.0, b.1),
        add(a.0, b.0),
        add(a.1, b.0),
        add(a.1, b.1),
    ];
    if signed_area(&r).abs() < 1e-7 {
        return None;
    }
    Some(Polygon::new(LineString::new(r), vec![]))
}

/// Convolve two closed point sequences, iterating edges with wrap-around.
fn convolve_rings(out: &mut Vec<Polygon<f64>>, a: &[Pt], b: &[Pt]) {
    let a = dedupe(a);
    let b = dedupe(b);
    if a.len() < 2 || b.len() < 2 {
        return;
    }
    for ia in 0..a.len() {
        let a_edge = (a[ia], a[(ia + 1) % a.len()]);
        for ib in 0..b.len() {
            let b_edge = (b[ib], b[(ib + 1) % b.len()]);
            if let Some(q) = convolve_two_segments(b_edge, a_edge) {
                out.push(q);
            }
        }
    }
}

/// Translate every ring of a polygon-with-holes by `d`.
fn translated(exterior: &[Pt], holes: &[Vec<Pt>], d: Pt) -> Polygon<f64> {
    let shift = |r: &[Pt]| -> LineString<f64> {
        r.iter()
            .map(|p| Coord { x: p.x + d.x, y: p.y + d.y })
            .collect()
    };
    Polygon::new(shift(exterior), holes.iter().map(|h| shift(h)).collect())
}

/// Compute the NFP of `b` around `a`. Returns the resolved polygon(s); for two
/// simple parts this is a single polygon, possibly with holes.
pub fn nfp(a: &Poly, b: &Poly) -> Vec<Poly> {
    // `a` outer ring + holes; `b` ring negated (the NFP is referenced to b[0]).
    let a_ext = a.points.clone();
    let a_holes: Vec<Vec<Pt>> = a.children.iter().map(|c| c.points.clone()).collect();

    let (xshift, yshift) = b
        .points
        .first()
        .map(|p| (p.x, p.y))
        .unwrap_or((0.0, 0.0));
    let b_ext: Vec<Pt> = b
        .points
        .iter()
        .map(|p| Pt::new(-p.x, -p.y))
        .collect();

    // Convolution: every ring of `a` against `b`, plus the vertex/polygon
    // translation terms (mirrors `convolve_two_polygon_sets`).
    let mut pieces: Vec<Polygon<f64>> = Vec::new();
    let a_rings = std::iter::once(&a_ext).chain(a_holes.iter());
    for ar in a_rings {
        convolve_rings(&mut pieces, ar, &b_ext);
    }
    if let Some(&b0) = b_ext.first() {
        pieces.push(translated(&a_ext, &a_holes, b0));
    }
    if let Some(&a0) = a_ext.first() {
        pieces.push(translated(&b_ext, &[], a0));
    }
    if pieces.is_empty() {
        return Vec::new();
    }

    // geo's boolean ops treat clockwise rings as holes, so orient every piece
    // CCW before dissolving them into clean polygons-with-holes.
    let oriented: Vec<Polygon<f64>> = pieces
        .into_iter()
        .map(|p| p.orient(Direction::Default))
        .collect();
    let resolved: MultiPolygon<f64> = geo::unary_union(oriented.iter());

    resolved
        .iter()
        .filter_map(|poly| poly_out(poly, xshift, yshift))
        .collect()
}

/// Convert a resolved `geo` polygon back to a [`Poly`], undoing the B-first
/// shift and dropping the redundant closing vertex.
fn poly_out(poly: &Polygon<f64>, xshift: f64, yshift: f64) -> Option<Poly> {
    let conv = |ls: &LineString<f64>| -> Vec<Pt> {
        let mut v: Vec<Pt> = ls
            .coords()
            .map(|c| Pt::new(c.x + xshift, c.y + yshift))
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
    Some(Poly {
        points,
        children: poly
            .interiors()
            .iter()
            .map(|h| Poly::from_points(conv(h)))
            .collect(),
        rotation: 0.0,
        source: -1,
        id: -1,
    })
}
