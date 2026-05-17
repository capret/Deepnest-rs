//! Polygon boolean operations — the replacement for Deepnest's in-page
//! ClipperLib usage (union of collision NFPs, difference against the sheet).
//!
//! Deepnest fed ClipperLib flat lists of paths with explicit fill rules;
//! here every polygon-with-holes is a structured [`geo::Polygon`], so the
//! "fill rule" is implicit (exterior minus interiors) and matches what the
//! NonZero/EvenOdd combinations produced for clean NFP polygons.

use crate::geom::{ring, Poly, Pt, TOL};
use geo::orient::{Direction, Orient};
use geo::{BooleanOps, LineString, MultiPolygon, Polygon};

/// Convert a [`Poly`] (outer ring + hole children) to an oriented `geo`
/// polygon. Orientation is normalised so booleans behave consistently.
fn to_geo(p: &Poly) -> Polygon<f64> {
    Polygon::new(
        ring(&p.points),
        p.children.iter().map(|c| ring(&c.points)).collect(),
    )
    .orient(Direction::Default)
}

fn to_geo_mp(polys: &[Poly]) -> MultiPolygon<f64> {
    MultiPolygon::new(polys.iter().map(to_geo).collect())
}

/// Convert a resolved `geo` polygon back to a [`Poly`], dropping the
/// duplicated closing vertex `geo` keeps on every ring.
fn ring_pts(ls: &LineString<f64>) -> Vec<Pt> {
    let mut v: Vec<Pt> = ls.coords().map(|c| Pt::new(c.x, c.y)).collect();
    if v.len() > 1 {
        let (f, l) = (v[0], v[v.len() - 1]);
        if (f.x - l.x).abs() < TOL && (f.y - l.y).abs() < TOL {
            v.pop();
        }
    }
    v
}

pub(crate) fn from_geo(poly: &Polygon<f64>) -> Option<Poly> {
    let points = ring_pts(poly.exterior());
    if points.len() < 3 {
        return None;
    }
    Some(Poly {
        points,
        children: poly
            .interiors()
            .iter()
            .map(|h| Poly::from_points(ring_pts(h)))
            .collect(),
        rotation: 0.0,
        source: -1,
        id: -1,
    })
}

/// Union of a list of polygons-with-holes (Clipper `ctUnion`, NonZero).
pub fn union(polys: &[Poly]) -> Vec<Poly> {
    if polys.is_empty() {
        return Vec::new();
    }
    let geos: Vec<Polygon<f64>> = polys.iter().map(to_geo).collect();
    geo::unary_union(geos.iter())
        .iter()
        .filter_map(from_geo)
        .collect()
}

/// `subject` minus `clip` (Clipper `ctDifference`).
pub fn difference(subject: &[Poly], clip: &[Poly]) -> Vec<Poly> {
    if subject.is_empty() {
        return Vec::new();
    }
    if clip.is_empty() {
        return subject.to_vec();
    }
    let s = to_geo_mp(subject);
    let c = to_geo_mp(clip);
    s.difference(&c).iter().filter_map(from_geo).collect()
}
