//! opennest-rs nesting engine.
//!
//! A native Rust port of Deepnest's `background.js`: it takes one genetic-
//! algorithm individual (a set of parts with chosen rotations, plus the
//! sheets) and returns a placement and fitness score. This replaces both the
//! C++ Minkowski addon and the in-page ClipperLib usage of the Electron app.
//!
//! Entry point: [`run`]. The crate is plain native Rust (no webview / wasm
//! toolchain) so the algorithm can be unit-tested directly — see `mod tests`.

pub mod clip;
pub mod geom;
pub mod nfp;
pub mod place;

pub use geom::{Poly, Pt};
pub use place::{place_parts, NestResult, NfpCache, Placement, SheetPlacement};

use serde::{Deserialize, Deserializer};

/// Nesting configuration — the subset of Deepnest's settings the placement
/// algorithm reads. Field names match the JS config keys.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "placementType", default = "default_placement")]
    pub placement_type: String,
    #[serde(rename = "mergeLines", default)]
    pub merge_lines: bool,
    #[serde(default)]
    pub scale: f64,
    #[serde(rename = "curveTolerance", default)]
    pub curve_tolerance: f64,
    #[serde(rename = "timeRatio", default)]
    pub time_ratio: f64,
    #[serde(default = "default_rotations")]
    pub rotations: f64,
    #[serde(default)]
    pub simplify: bool,
}

fn default_placement() -> String {
    "box".to_string()
}
fn default_rotations() -> f64 {
    4.0
}

/// Deserialize a `Vec` field, tolerating both a missing key and an explicit
/// `null`. Deepnest's renderer hands us arrays whose entries are frequently
/// `null`/`undefined` (a part or sheet with no holes, an unset id, …); plain
/// serde would reject those.
fn de_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// One GA individual: the parts to place and the rotation chosen for each.
#[derive(Debug, Deserialize)]
pub struct Individual {
    #[serde(default, deserialize_with = "de_vec")]
    pub placement: Vec<Vec<Pt>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub rotation: Vec<Option<f64>>,
}

/// The full `background-start` payload Deepnest's renderer sends per individual.
///
/// Numeric ids are decoded as `f64` (JS numbers are floating point) and the
/// `Option` element types absorb the `null`s the renderer scatters through
/// these arrays; both are normalised when parts/sheets are built.
#[derive(Debug, Deserialize)]
pub struct NestInput {
    #[serde(default)]
    pub index: f64,
    #[serde(default, deserialize_with = "de_vec")]
    pub sheets: Vec<Vec<Pt>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub sheetids: Vec<Option<f64>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub sheetsources: Vec<Option<f64>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub sheetchildren: Vec<Option<Vec<Vec<Pt>>>>,
    pub individual: Individual,
    pub config: Config,
    #[serde(default, deserialize_with = "de_vec")]
    pub ids: Vec<Option<f64>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub sources: Vec<Option<f64>>,
    #[serde(default, deserialize_with = "de_vec")]
    pub children: Vec<Option<Vec<Vec<Pt>>>>,
}

fn holes(children: &[Option<Vec<Vec<Pt>>>], i: usize) -> Vec<Poly> {
    children
        .get(i)
        .and_then(|o| o.as_ref())
        .map(|hs| hs.iter().cloned().map(Poly::from_points).collect())
        .unwrap_or_default()
}

/// Run the nesting computation for one GA individual.
///
/// `cache` is the persistent NFP cache (share it across calls so repeated
/// part pairs are computed once). `progress` is called with values in `0..=1`
/// during placement and finally `-1.0` when finished.
pub fn run(input: NestInput, cache: &mut NfpCache, progress: impl FnMut(f64)) -> NestResult {
    let config = input.config.clone();
    let index = input.index as i64;

    // build parts from the GA individual
    let mut parts: Vec<Poly> = Vec::with_capacity(input.individual.placement.len());
    for (i, pts) in input.individual.placement.iter().enumerate() {
        let mut poly = Poly::from_points(pts.clone());
        poly.rotation = input.individual.rotation.get(i).copied().flatten().unwrap_or(0.0);
        poly.id = input.ids.get(i).copied().flatten().unwrap_or(-1.0) as i64;
        poly.source = input.sources.get(i).copied().flatten().unwrap_or(-1.0) as i64;
        if !config.simplify {
            poly.children = holes(&input.children, i);
        }
        parts.push(poly);
    }

    // build sheets
    let mut sheets: Vec<Poly> = Vec::with_capacity(input.sheets.len());
    for (i, pts) in input.sheets.iter().enumerate() {
        let mut poly = Poly::from_points(pts.clone());
        poly.id = input.sheetids.get(i).copied().flatten().unwrap_or(-1.0) as i64;
        poly.source = input.sheetsources.get(i).copied().flatten().unwrap_or(-1.0) as i64;
        poly.children = holes(&input.sheetchildren, i);
        sheets.push(poly);
    }

    let mut result = place_parts(sheets, parts, &config, cache, progress);
    result.index = index;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::{difference, union};
    use crate::geom::{get_polygon_bounds, polygon_area};
    use crate::nfp::nfp;

    fn rect(w: f64, h: f64) -> Poly {
        Poly::from_points(vec![
            Pt::new(0.0, 0.0),
            Pt::new(w, 0.0),
            Pt::new(w, h),
            Pt::new(0.0, h),
        ])
    }

    #[test]
    fn polygon_area_is_signed() {
        // 10x10 square; the original geometryutil sign convention is preserved.
        let a = polygon_area(&rect(10.0, 10.0).points);
        assert!((a.abs() - 100.0).abs() < 1e-6, "area was {a}");
    }

    #[test]
    fn nfp_of_two_squares() {
        // NFP of a 5x5 around a 10x10 is the 15x15 Minkowski sum, [-5,10]^2.
        let out = nfp(&rect(10.0, 10.0), &rect(5.0, 5.0));
        assert_eq!(out.len(), 1, "expected a single NFP polygon");
        let b = get_polygon_bounds(&out[0].points).unwrap();
        assert!((b.x - -5.0).abs() < 1e-6, "x min {}", b.x);
        assert!((b.width - 15.0).abs() < 1e-6, "width {}", b.width);
        assert!((b.height - 15.0).abs() < 1e-6, "height {}", b.height);
    }

    #[test]
    fn union_merges_overlapping_squares() {
        let mut r2 = rect(10.0, 10.0);
        r2.points = r2.points.iter().map(|p| Pt::new(p.x + 5.0, p.y)).collect();
        let u = union(&[rect(10.0, 10.0), r2]);
        assert_eq!(u.len(), 1, "overlapping squares should merge");
        let b = get_polygon_bounds(&u[0].points).unwrap();
        assert!((b.width - 15.0).abs() < 1e-6, "width {}", b.width);
    }

    #[test]
    fn difference_cuts_a_hole() {
        // big square minus a centered small square -> a polygon with one hole
        let mut small = rect(4.0, 4.0);
        small.points = small
            .points
            .iter()
            .map(|p| Pt::new(p.x + 3.0, p.y + 3.0))
            .collect();
        let d = difference(&[rect(10.0, 10.0)], &[small]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].children.len(), 1, "expected one hole");
    }

    #[test]
    fn places_parts_on_a_sheet() {
        // four 2x2 parts onto one 10x10 sheet — all should be placed.
        let config = Config {
            placement_type: "box".to_string(),
            merge_lines: false,
            scale: 72.0,
            curve_tolerance: 0.72,
            time_ratio: 0.5,
            rotations: 4.0,
            simplify: true,
        };
        let mut parts = Vec::new();
        for id in 0..4 {
            let mut p = rect(2.0, 2.0);
            p.id = id;
            p.source = id;
            parts.push(p);
        }
        let mut sheet = rect(10.0, 10.0);
        sheet.id = 100;
        sheet.source = 100;

        let mut cache = NfpCache::default();
        let result = place_parts(vec![sheet], parts, &config, &mut cache, |_| {});
        let placed: usize = result
            .placements
            .iter()
            .map(|s| s.sheetplacements.len())
            .sum();
        assert_eq!(placed, 4, "all four parts should fit on the sheet");
        assert!(result.fitness > 0.0, "fitness should be positive");
    }

    #[test]
    fn deserializes_payload_with_nulls() {
        // Mirrors what deepnest.js sends: arrays sprinkled with `null` for
        // parts/sheets with no holes and for unset ids.
        let json = r#"{
            "index": 3,
            "sheets": [[{"x":0,"y":0},{"x":50,"y":0},{"x":50,"y":50},{"x":0,"y":50}]],
            "sheetids": [100],
            "sheetsources": [100],
            "sheetchildren": [null],
            "individual": {
                "placement": [
                    [{"x":0,"y":0},{"x":4,"y":0},{"x":4,"y":4},{"x":0,"y":4}],
                    [{"x":0,"y":0},{"x":4,"y":0},{"x":4,"y":4},{"x":0,"y":4}]
                ],
                "rotation": [0, null]
            },
            "config": {"placementType":"box","mergeLines":false,"rotations":4},
            "ids": [0, 1],
            "sources": [0, null],
            "children": [null, null]
        }"#;
        let input: NestInput =
            serde_json::from_str(json).expect("payload with nulls should deserialize");
        let mut cache = NfpCache::default();
        let result = run(input, &mut cache, |_| {});
        assert_eq!(result.index, 3);
        let placed: usize = result
            .placements
            .iter()
            .map(|s| s.sheetplacements.len())
            .sum();
        assert_eq!(placed, 2, "both parts should be placed");
    }
}
