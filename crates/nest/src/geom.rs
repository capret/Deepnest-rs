//! Core geometry types and primitives.
//!
//! These are faithful Rust ports of the helpers Deepnest used from
//! `geometryutil.js` and `background.js` (polygon area, bounds, rotation,
//! convex hull, …). Sign conventions are kept identical to the originals so
//! every `area > 0` / `area < 0` test downstream behaves the same.

use geo::{ConvexHull, Coord, LineString, MultiPoint, Point as GeoPoint};
use serde::{Deserialize, Serialize};

/// Floating-point tolerance, matching geometryutil.js `TOL`.
pub const TOL: f64 = 1e-9;

/// A 2D point. `exact` marks a vertex that came from an original CAD line (as
/// opposed to an approximated curve point); only exact segments are eligible
/// for line merging.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub exact: bool,
}

impl Pt {
    pub fn new(x: f64, y: f64) -> Self {
        Pt { x, y, exact: false }
    }
}

/// A polygon with optional holes (`children`) and nesting metadata.
///
/// `source` identifies the originating shape (used as the NFP-cache key);
/// `id` identifies a specific placed instance. Both are `-1` when absent.
#[derive(Debug, Clone, Default)]
pub struct Poly {
    pub points: Vec<Pt>,
    pub children: Vec<Poly>,
    pub rotation: f64,
    pub source: i64,
    pub id: i64,
}

impl Poly {
    pub fn from_points(points: Vec<Pt>) -> Self {
        Poly {
            points,
            children: Vec::new(),
            rotation: 0.0,
            source: -1,
            id: -1,
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Axis-aligned bounding box, mirroring `getPolygonBounds`.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// `geometryutil.js` `almostEqual`.
pub fn almost_equal(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

/// `geometryutil.js` `polygonArea`. Positive for clockwise polygons; the sign
/// is relied on by the NFP/clipper code, so the formula is kept verbatim.
pub fn polygon_area(poly: &[Pt]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        area += (poly[j].x + poly[i].x) * (poly[j].y - poly[i].y);
        j = i;
    }
    0.5 * area
}

/// `geometryutil.js` `getPolygonBounds`.
pub fn get_polygon_bounds(poly: &[Pt]) -> Option<Bounds> {
    if poly.is_empty() {
        return None;
    }
    let mut xmin = poly[0].x;
    let mut xmax = poly[0].x;
    let mut ymin = poly[0].y;
    let mut ymax = poly[0].y;
    for p in poly {
        xmin = xmin.min(p.x);
        xmax = xmax.max(p.x);
        ymin = ymin.min(p.y);
        ymax = ymax.max(p.y);
    }
    Some(Bounds {
        x: xmin,
        y: ymin,
        width: xmax - xmin,
        height: ymax - ymin,
    })
}

/// Rotate a polygon (and its holes) by `degrees` about the origin. Mirrors
/// `background.js` `rotatePolygon`; `exact` flags are preserved.
pub fn rotate_polygon(poly: &Poly, degrees: f64) -> Poly {
    let angle = degrees * std::f64::consts::PI / 180.0;
    let (sin, cos) = angle.sin_cos();
    let points = poly
        .points
        .iter()
        .map(|p| Pt {
            x: p.x * cos - p.y * sin,
            y: p.x * sin + p.y * cos,
            exact: p.exact,
        })
        .collect();
    Poly {
        points,
        children: poly
            .children
            .iter()
            .map(|c| rotate_polygon(c, degrees))
            .collect(),
        rotation: poly.rotation,
        source: poly.source,
        id: poly.id,
    }
}

/// Translate a polygon (and its holes) by `dx, dy`. Mirrors `shiftPolygon`.
pub fn shift_polygon(poly: &Poly, dx: f64, dy: f64) -> Poly {
    Poly {
        points: poly
            .points
            .iter()
            .map(|p| Pt {
                x: p.x + dx,
                y: p.y + dy,
                exact: p.exact,
            })
            .collect(),
        children: poly
            .children
            .iter()
            .map(|c| shift_polygon(c, dx, dy))
            .collect(),
        rotation: poly.rotation,
        source: poly.source,
        id: poly.id,
    }
}

/// Convex hull of a point set, mirroring `getHull` (d3.polygonHull). Falls
/// back to the input when a hull cannot be formed (fewer than 3 points).
pub fn get_hull(points: &[Pt]) -> Vec<Pt> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mp: MultiPoint<f64> = points
        .iter()
        .map(|p| GeoPoint::new(p.x, p.y))
        .collect();
    let hull = mp.convex_hull();
    let mut out: Vec<Pt> = hull
        .exterior()
        .coords()
        .map(|c| Pt::new(c.x, c.y))
        .collect();
    // geo closes the ring; drop the duplicated last vertex.
    if out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if almost_equal(f.x, l.x, TOL) && almost_equal(f.y, l.y, TOL) {
            out.pop();
        }
    }
    if out.len() < 3 {
        points.to_vec()
    } else {
        out
    }
}

/// Build a closed `geo` ring from a point list.
pub(crate) fn ring(points: &[Pt]) -> LineString<f64> {
    LineString::new(points.iter().map(|p| Coord { x: p.x, y: p.y }).collect())
}
