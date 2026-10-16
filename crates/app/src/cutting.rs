//! Export touching cut edges once, including partial collinear overlaps.
use crate::model::{transformed, PlacedPart, Project};
use nest::Pt;
use std::collections::BTreeMap;

pub fn shared_cuts(project: &Project) -> bool {
    project.settings.merge_lines && project.stock.gap == 0.
}

pub fn segments(project: &Project, sheet: &[PlacedPart]) -> Vec<[Pt; 2]> {
    let mut edges = vec![];
    for placement in sheet {
        let part = &project.parts[placement.part];
        for ring in std::iter::once(&part.points).chain(&part.holes) {
            for (a, b) in ring
                .iter()
                .zip(ring.iter().cycle().skip(1))
                .take(ring.len())
            {
                edges.push([transformed(*a, placement), transformed(*b, placement)]);
            }
        }
    }
    union_segments(edges)
}

fn union_segments(edges: Vec<[Pt; 2]>) -> Vec<[Pt; 2]> {
    // Group the same supporting line. This microscopic tolerance handles roundoff
    // from rotations; it never bridges the user-defined spacing between parts.
    const EPS: f64 = 1e-7;
    type Line = (Pt, Pt, Vec<(f64, f64)>);
    let mut lines: BTreeMap<(i64, i64, i64), Line> = BTreeMap::new();
    for [a, b] in edges {
        let length = (b.x - a.x).hypot(b.y - a.y);
        if length < EPS {
            continue;
        }
        let mut u = Pt::new((b.x - a.x) / length, (b.y - a.y) / length);
        if u.x < -EPS || (u.x.abs() <= EPS && u.y < 0.) {
            u.x = -u.x;
            u.y = -u.y;
        }
        let offset = -u.y * a.x + u.x * a.y;
        let key = (
            (u.x / EPS).round() as i64,
            (u.y / EPS).round() as i64,
            (offset / EPS).round() as i64,
        );
        let line = lines.entry(key).or_insert((a, u, vec![]));
        let project = |p: Pt| (p.x - line.0.x) * line.1.x + (p.y - line.0.y) * line.1.y;
        let (start, end) = (project(a), project(b));
        line.2.push((start.min(end), start.max(end)));
    }
    let mut result = vec![];
    for (_, (origin, u, mut intervals)) in lines {
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = vec![];
        for (start, end) in intervals {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1 + EPS) {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        for (start, end) in merged {
            result.push([
                Pt::new(origin.x + u.x * start, origin.y + u.y * start),
                Pt::new(origin.x + u.x * end, origin.y + u.y * end),
            ]);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_and_dxf_exports_remove_the_same_shared_cut() {
        let mut project = Project::default();
        project.stock.gap = 0.;
        project.parts.push(crate::model::Part {
            name: "Square".into(),
            points: vec![
                Pt::new(0., 0.),
                Pt::new(20., 0.),
                Pt::new(20., 20.),
                Pt::new(0., 20.),
            ],
            holes: vec![],
            quantity: 2,
        });
        let layout = crate::model::Layout {
            sheets: vec![vec![
                PlacedPart {
                    part: 0,
                    instance: 0,
                    x: 0.,
                    y: 0.,
                    rotation: 0.,
                },
                PlacedPart {
                    part: 0,
                    instance: 1,
                    x: 20.,
                    y: 0.,
                    rotation: 0.,
                },
            ]],
            fitness: 0.,
            elapsed: 0.,
            pass: 1,
        };
        let svg = crate::svg_io::export(&project, &layout);
        assert_eq!(svg.matches(" L ").count(), 5);
        let output = crate::dxf_io::export(&project, &layout).unwrap();
        let drawing = dxf::Drawing::load(&mut std::io::Cursor::new(output.as_bytes())).unwrap();
        let mut length = 0.;
        for e in drawing.entities() {
            let dxf::entities::EntityType::Line(line) = &e.specific else {
                panic!("Expected individual cut segments");
            };
            length += (line.p2.x - line.p1.x).hypot(line.p2.y - line.p1.y);
        }
        assert!((length - 140.).abs() < 1e-5);
        project.settings.merge_lines = false;
        assert_eq!(
            crate::svg_io::export(&project, &layout)
                .matches("<path")
                .count(),
            2
        );
        let output = crate::dxf_io::export(&project, &layout).unwrap();
        let drawing = dxf::Drawing::load(&mut std::io::Cursor::new(output.as_bytes())).unwrap();
        assert_eq!(drawing.entities().count(), 2);
        assert!(drawing.entities().all(
            |e| matches!(&e.specific, dxf::entities::EntityType::LwPolyline(p) if p.is_closed())
        ));
    }
    #[test]
    fn shared_and_partial_edges_are_cut_once_without_bridging_gaps() {
        let edges = union_segments(vec![
            [Pt::new(0., 0.), Pt::new(10., 0.)],
            [Pt::new(15., 0.), Pt::new(5., 0.)],
            [Pt::new(0., 0.01), Pt::new(10., 0.01)],
        ]);
        assert_eq!(edges.len(), 2);
        let length: f64 = edges
            .iter()
            .map(|[a, b]| (b.x - a.x).hypot(b.y - a.y))
            .sum();
        assert!((length - 25.).abs() < 1e-7);
    }
}
