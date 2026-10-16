//! SVG is parsed by usvg (shapes, paths, transforms, viewBox and units), then
//! flattened to millimeter polygons using the project’s curve tolerance. No webview is used.
use crate::contours::{assemble, finish, Contour, Import};
use crate::model::{transformed, Layout, Project};
use crate::settings::Settings;
use lyon_geom::{point, CubicBezierSegment, QuadraticBezierSegment};
use nest::Pt;
use std::fmt::Write;
use usvg::{tiny_skia_path::PathSegment, Node};

#[cfg(test)]
const MM_PER_PX: f64 = 25.4 / 96.0;

#[cfg(test)]
fn parse(source: &str, filename: &str) -> Result<Import, String> {
    parse_with(source, filename, &Settings::default())
}

pub fn parse_with(source: &str, filename: &str, settings: &Settings) -> Result<Import, String> {
    settings.validate()?;
    let options = usvg::Options {
        dpi: settings.svg_dpi as f32,
        ..Default::default()
    };
    let tree =
        usvg::Tree::from_str(source, &options).map_err(|e| format!("Cannot read SVG: {e}"))?;
    let mut contours = vec![];
    let mut skipped = 0;
    let mut open = vec![];
    collect(
        tree.root(),
        settings,
        &mut contours,
        &mut open,
        &mut skipped,
    );
    crate::contours::stitch(
        open,
        &mut contours,
        &mut skipped,
        settings.endpoint_tolerance,
    );
    if contours.is_empty() {
        return Err("No closed vector shapes found. Convert text to outlines and close open paths before importing.".into());
    }
    assemble(contours, filename, skipped)
}

fn collect(
    group: &usvg::Group,
    settings: &Settings,
    contours: &mut Vec<Contour>,
    open: &mut Vec<Contour>,
    skipped: &mut usize,
) {
    let mm_per_px = 25.4 / settings.svg_dpi;
    // A clipped outline differs from its source path. Do not silently turn it
    // into a different cutting shape; ask the source editor to flatten effects.
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        *skipped += 1;
        return;
    }
    for node in group.children() {
        match node {
            Node::Group(group) => collect(group, settings, contours, open, skipped),
            Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                let Some(data) = path.data().clone().transform(path.abs_transform()) else {
                    continue;
                };
                let convert = |p: usvg::tiny_skia_path::Point| {
                    Pt::new(p.x as f64 * mm_per_px, p.y as f64 * mm_per_px)
                };
                let mut points = vec![];
                let mut current = Pt::default();
                for segment in data.segments() {
                    match segment {
                        PathSegment::MoveTo(p) => {
                            finish_path(&mut points, false, path.id(), contours, open, skipped);
                            current = convert(p);
                            points.push(current);
                        }
                        PathSegment::LineTo(p) => {
                            if let Some(last) = points.last_mut() {
                                last.exact = true;
                            }
                            current = convert(p);
                            current.exact = true;
                            points.push(current);
                        }
                        PathSegment::QuadTo(control, end) => {
                            let c = convert(control);
                            let end = convert(end);
                            let curve = QuadraticBezierSegment {
                                from: point(current.x, current.y),
                                ctrl: point(c.x, c.y),
                                to: point(end.x, end.y),
                            };
                            curve.for_each_flattened(settings.curve_tolerance, &mut |line| {
                                points.push(Pt::new(line.to.x, line.to.y))
                            });
                            current = end;
                        }
                        PathSegment::CubicTo(c1, c2, end) => {
                            let (c1, c2, end) = (convert(c1), convert(c2), convert(end));
                            let curve = CubicBezierSegment {
                                from: point(current.x, current.y),
                                ctrl1: point(c1.x, c1.y),
                                ctrl2: point(c2.x, c2.y),
                                to: point(end.x, end.y),
                            };
                            curve.for_each_flattened(settings.curve_tolerance, &mut |line| {
                                points.push(Pt::new(line.to.x, line.to.y))
                            });
                            current = end;
                        }
                        PathSegment::Close => {
                            finish_path(&mut points, true, path.id(), contours, open, skipped)
                        }
                    }
                }
                finish_path(&mut points, false, path.id(), contours, open, skipped);
            }
            _ => *skipped += 1,
        }
    }
}

fn finish_path(
    points: &mut Vec<Pt>,
    closed: bool,
    name: &str,
    contours: &mut Vec<Contour>,
    open: &mut Vec<Contour>,
    skipped: &mut usize,
) {
    if !closed && points.len() >= 2 {
        open.push(Contour {
            name: name.into(),
            points: std::mem::take(points),
        });
    } else {
        finish(points, closed, name, contours, skipped);
    }
}

pub fn export(project: &Project, layout: &Layout) -> String {
    let width = project.stock.width;
    let height = project.stock.height * layout.sheets.len() as f64
        + 20.0 * layout.sheets.len().saturating_sub(1) as f64;
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">\n<title>Deepnest cutting layout</title>\n");
    for (i, sheet) in layout.sheets.iter().enumerate() {
        let offset = i as f64 * (project.stock.height + 20.0);
        let _ = writeln!(out, "<g id=\"sheet-{}\" data-width-mm=\"{}\" data-height-mm=\"{}\" transform=\"translate(0 {offset})\" fill=\"none\" stroke=\"black\" stroke-width=\"0.1\">", i+1, width, project.stock.height);
        if crate::cutting::shared_cuts(project) {
            out.push_str("<path d=\"");
            for [a, b] in crate::cutting::segments(project, sheet) {
                let _ = write!(out, "M {:.7} {:.7} L {:.7} {:.7} ", a.x, a.y, b.x, b.y);
            }
            out.push_str("\"/>\n</g>\n");
            continue;
        }
        for placement in sheet {
            let part = &project.parts[placement.part];
            let _ = write!(out, "<path id=\"part-{}\" d=\"", placement.instance);
            for ring in std::iter::once(&part.points).chain(&part.holes) {
                for (j, p) in ring.iter().enumerate() {
                    let p = transformed(*p, placement);
                    let _ = write!(
                        out,
                        "{} {:.5} {:.5} ",
                        if j == 0 { "M" } else { "L" },
                        p.x,
                        p.y
                    );
                }
                out.push_str("Z ");
            }
            out.push_str("\"/>\n");
        }
        out.push_str("</g>\n");
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_scale_changes_unitless_files_but_preserves_physical_dimensions() {
        let unitless = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96"/></svg>"#;
        let physical = r#"<svg xmlns="http://www.w3.org/2000/svg" width="25.4mm" height="25.4mm" viewBox="0 0 96 96"><rect width="96" height="96"/></svg>"#;
        for dpi in [72., 90., 96., 144.] {
            let settings = Settings {
                svg_dpi: dpi,
                ..Settings::default()
            };
            let a = parse_with(unitless, "unitless.svg", &settings).unwrap();
            let b = parse_with(physical, "physical.svg", &settings).unwrap();
            assert!((a.parts[0].bounds().width - 96. * 25.4 / dpi).abs() < 1e-4);
            assert!((b.parts[0].bounds().width - 25.4).abs() < 1e-4);
        }
    }
    #[test]
    fn configured_curve_and_endpoint_tolerances_change_import_geometry() {
        let circle = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><circle r="40" cx="50" cy="50"/></svg>"#;
        let fine = parse_with(
            circle,
            "fine.svg",
            &Settings {
                curve_tolerance: 0.01,
                ..Settings::default()
            },
        )
        .unwrap();
        let coarse = parse_with(
            circle,
            "coarse.svg",
            &Settings {
                curve_tolerance: 0.5,
                ..Settings::default()
            },
        )
        .unwrap();
        assert!(fine.parts[0].points.len() > coarse.parts[0].points.len());
        let gap = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><path d="M0 0 L20 0 L20 20 L0 20 L0 0.02" fill="none" stroke="black"/></svg>"#;
        assert!(parse_with(gap, "gap.svg", &Settings::default()).is_err());
        let repaired = parse_with(
            gap,
            "gap.svg",
            &Settings {
                endpoint_tolerance: 0.05,
                ..Settings::default()
            },
        )
        .unwrap();
        assert!((repaired.parts[0].area() - 400.).abs() < 1e-3);
    }
    #[test]
    fn imports_physical_units_viewbox_and_nested_transforms() {
        let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50"><g transform="translate(10 2)"><g transform="scale(2)"><rect id="plate" width="20" height="10"/></g></g></svg>"#;
        let imported = parse(input, "part.svg").unwrap();
        assert_eq!(imported.parts.len(), 1);
        let bounds = imported.parts[0].bounds();
        assert!((bounds.width - 40.0).abs() < 1e-4);
        assert!((bounds.height - 20.0).abs() < 1e-4);
        assert!(bounds.x.abs() < 1e-6);
    }
    #[test]
    fn preserves_holes_and_flattens_curves() {
        let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle r="40" cx="50" cy="50"/><circle r="15" cx="50" cy="50"/></svg>"#;
        let imported = parse(input, "ring.svg").unwrap();
        assert_eq!(imported.parts.len(), 1);
        assert_eq!(imported.parts[0].holes.len(), 1);
        assert!(imported.parts[0].points.len() > 16);
        let area = std::f64::consts::PI * (1600.0 - 225.0) * MM_PER_PX.powi(2);
        assert!((imported.parts[0].area() / area - 1.0).abs() < 0.02);
    }
    #[test]
    fn rejects_open_paths_instead_of_inventing_cutting_edges() {
        let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0 L50 0 L50 50" fill="none" stroke="black"/></svg>"#;
        assert!(parse(input, "open.svg").is_err());
    }
    #[test]
    fn export_round_trip_preserves_cut_size_and_holes() {
        let project = Project::demo();
        let layout = Layout {
            sheets: vec![vec![crate::model::PlacedPart {
                part: 0,
                instance: 0,
                x: 150.,
                y: 10.,
                rotation: 90.,
            }]],
            fitness: 1.,
            elapsed: 0.,
            pass: 1,
        };
        let exported = export(&project, &layout);
        let imported = parse(&exported, "roundtrip.svg").unwrap();
        assert_eq!(imported.parts.len(), 1);
        assert_eq!(imported.parts[0].holes.len(), 4);
        assert!((imported.parts[0].bounds().width - 80.0).abs() < 1e-3);
        assert!((imported.parts[0].area() - project.parts[0].area()).abs() < 0.01);
    }
}
