//! Local DXF geometry, converted from CAD's Y-up coordinates to canvas Y-down.
use crate::settings::{CadUnit, Settings};
use crate::{
    contours::{self, Contour, Import},
    model::{transformed, Layout, Project},
};
use dxf::{
    entities::{Entity, EntityType, Line, LwPolyline},
    enums::{AcadVersion, Units},
    Drawing, LwPolylineVertex,
};
use nest::Pt;
use std::f64::consts::TAU;

#[derive(Clone, Copy)]
struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    x: f64,
    y: f64,
}
impl Transform {
    const IDENTITY: Self = Self {
        a: 1.,
        b: 0.,
        c: 0.,
        d: 1.,
        x: 0.,
        y: 0.,
    };
    fn point(self, p: Pt) -> Pt {
        Pt {
            x: self.a * p.x + self.c * p.y + self.x,
            y: self.b * p.x + self.d * p.y + self.y,
            exact: p.exact,
        }
    }
    fn then(self, t: Self) -> Self {
        let origin = self.point(Pt::new(t.x, t.y));
        Self {
            a: self.a * t.a + self.c * t.b,
            b: self.b * t.a + self.d * t.b,
            c: self.a * t.c + self.c * t.d,
            d: self.b * t.c + self.d * t.d,
            x: origin.x,
            y: origin.y,
        }
    }
    fn scale_bound(self) -> f64 {
        (self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d).sqrt()
    }
}
struct Collector<'a> {
    drawing: &'a Drawing,
    scale: f64,
    closed: Vec<Contour>,
    open: Vec<Contour>,
    tolerance: f64,
    join_tolerance: f64,
    skipped: usize,
    count: usize,
}

#[cfg(test)]
fn parse(bytes: &[u8], filename: &str) -> Result<Import, String> {
    parse_with(bytes, filename, &Settings::default())
}

pub fn parse_with(bytes: &[u8], filename: &str, settings: &Settings) -> Result<Import, String> {
    settings.validate()?;
    let drawing = Drawing::load(&mut std::io::Cursor::new(bytes))
        .map_err(|e| format!("Cannot read DXF: {e}"))?;
    let scale = match settings.dxf_import.millimeters() {
        Some(scale) => scale,
        None => millimeters(drawing.header.default_drawing_units)?,
    };
    let mut collector = Collector {
        drawing: &drawing,
        scale,
        tolerance: settings.curve_tolerance,
        join_tolerance: settings.endpoint_tolerance,
        closed: vec![],
        open: vec![],
        skipped: 0,
        count: 0,
    };
    for entity in drawing.entities() {
        collector.entity(entity, Transform::IDENTITY, 0)?;
    }
    contours::stitch(
        collector.open,
        &mut collector.closed,
        &mut collector.skipped,
        settings.endpoint_tolerance,
    );
    let mut imported = contours::assemble(collector.closed, filename, collector.skipped)?;
    imported.assumed_mm = settings.dxf_import == CadUnit::Auto
        && drawing.header.default_drawing_units == Units::Unitless;
    Ok(imported)
}

fn millimeters(unit: Units) -> Result<f64, String> {
    Ok(match unit {
        Units::Unitless | Units::Millimeters => 1.,
        Units::Inches => 25.4,
        Units::Feet => 304.8,
        Units::Miles => 1_609_344.,
        Units::Centimeters => 10.,
        Units::Meters => 1000.,
        Units::Kilometers => 1_000_000.,
        Units::Microinches => 0.0000254,
        Units::Mils => 0.0254,
        Units::Yards => 914.4,
        Units::Angstroms => 1e-7,
        Units::Nanometers => 1e-6,
        Units::Microns => 0.001,
        Units::Decimeters => 100.,
        Units::Decameters => 10_000.,
        Units::Hectometers => 100_000.,
        Units::Gigameters => 1e12,
        Units::USSurveyFeet => 1_200_000. / 3937.,
        Units::USSurveyInch => 100_000. / 3937.,
        Units::USSurveyYard => 3_600_000. / 3937.,
        Units::USSurveyMile => 6_336_000_000. / 3937.,
        _ => return Err("Convert astronomical DXF units to millimeters before importing.".into()),
    })
}
fn plane(normal: &dxf::Vector) -> Option<Transform> {
    (normal.x.abs() < 1e-8 && normal.y.abs() < 1e-8 && (normal.z.abs() - 1.).abs() < 1e-8)
        .then_some(Transform {
            a: normal.z.signum(),
            ..Transform::IDENTITY
        })
}

impl Collector<'_> {
    fn entity(&mut self, e: &Entity, t: Transform, depth: usize) -> Result<(), String> {
        self.count += 1;
        if self.count > 50_000 {
            return Err("DXF contains too many entities. Split it into smaller files.".into());
        }
        if !e.common.is_visible || e.common.is_in_paper_space {
            self.skipped += 1;
            return Ok(());
        }
        let name = if e.common.layer == "0" {
            String::new()
        } else {
            e.common.layer.clone()
        };
        let tolerance = self.tolerance / (self.scale * t.scale_bound()).max(1e-12);
        match &e.specific {
            EntityType::Line(line) if (line.p1.z - line.p2.z).abs() < 1e-8 => {
                self.add(
                    name,
                    vec![
                        Pt {
                            x: line.p1.x,
                            y: line.p1.y,
                            exact: true,
                        },
                        Pt {
                            x: line.p2.x,
                            y: line.p2.y,
                            exact: true,
                        },
                    ],
                    false,
                    t,
                )?;
            }
            EntityType::Circle(circle) if circle.radius > 0. && plane(&circle.normal).is_some() => {
                let points = arc(
                    Pt::new(circle.center.x, circle.center.y),
                    circle.radius,
                    0.,
                    TAU,
                    tolerance,
                )?;
                self.add(name, points, true, t.then(plane(&circle.normal).unwrap()))?;
            }
            EntityType::Arc(a) if a.radius > 0. && plane(&a.normal).is_some() => {
                let sweep = (a.end_angle - a.start_angle).to_radians().rem_euclid(TAU);
                if sweep < 1e-10 {
                    self.skipped += 1;
                } else {
                    let points = arc(
                        Pt::new(a.center.x, a.center.y),
                        a.radius,
                        a.start_angle.to_radians(),
                        sweep,
                        tolerance,
                    )?;
                    self.add(name, points, false, t.then(plane(&a.normal).unwrap()))?;
                }
            }
            EntityType::LwPolyline(p) if plane(&p.extrusion_direction).is_some() => {
                let vertices: Vec<_> = p
                    .vertices
                    .iter()
                    .map(|v| (Pt::new(v.x, v.y), v.bulge))
                    .collect();
                self.add(
                    name,
                    polyline(&vertices, p.is_closed(), tolerance)?,
                    p.is_closed(),
                    t.then(plane(&p.extrusion_direction).unwrap()),
                )?;
            }
            EntityType::Polyline(p)
                if p.flags & (2 | 4 | 8 | 16 | 64) == 0 && plane(&p.normal).is_some() =>
            {
                let vertices: Vec<_> = p
                    .vertices()
                    .map(|v| (Pt::new(v.location.x, v.location.y), v.bulge))
                    .collect();
                self.add(
                    name,
                    polyline(&vertices, p.is_closed(), tolerance)?,
                    p.is_closed(),
                    t.then(plane(&p.normal).unwrap()),
                )?;
            }
            EntityType::Ellipse(e)
                if plane(&e.normal).is_some()
                    && e.major_axis.z.abs() < 1e-8
                    && e.minor_axis_ratio > 0.
                    && e.minor_axis_ratio <= 1. =>
            {
                let radius = e.major_axis.x.hypot(e.major_axis.y);
                let full = (e.end_parameter - e.start_parameter).abs() >= TAU - 1e-8;
                let sweep = if full {
                    TAU
                } else {
                    (e.end_parameter - e.start_parameter).rem_euclid(TAU)
                };
                let points = arc(
                    Pt::new(0., 0.),
                    1.,
                    e.start_parameter,
                    sweep,
                    tolerance / radius.max(1e-12),
                )?
                .into_iter()
                .map(|p| {
                    let minor = p.y * e.minor_axis_ratio * e.normal.z.signum();
                    Pt::new(
                        e.center.x + e.major_axis.x * p.x - e.major_axis.y * minor,
                        e.center.y + e.major_axis.y * p.x + e.major_axis.x * minor,
                    )
                })
                .collect();
                self.add(name, points, full, t)?;
            }
            EntityType::Insert(insert)
                if depth < 16
                    && insert.column_count <= 1
                    && insert.row_count <= 1
                    && plane(&insert.extrusion_direction).is_some() =>
            {
                if let Some(block) = self.drawing.blocks().find(|b| b.name == insert.name) {
                    if block.is_xref() {
                        self.skipped += 1;
                        return Ok(());
                    }
                    let (sin, cos) = insert.rotation.to_radians().sin_cos();
                    let mut local = Transform {
                        a: cos * insert.x_scale_factor,
                        b: sin * insert.x_scale_factor,
                        c: -sin * insert.y_scale_factor,
                        d: cos * insert.y_scale_factor,
                        x: 0.,
                        y: 0.,
                    };
                    let base = local.point(Pt::new(block.base_point.x, block.base_point.y));
                    local.x = insert.location.x - base.x;
                    local.y = insert.location.y - base.y;
                    let nested = t
                        .then(plane(&insert.extrusion_direction).unwrap())
                        .then(local);
                    for entity in &block.entities {
                        self.entity(entity, nested, depth + 1)?;
                    }
                } else {
                    self.skipped += 1;
                }
            }
            _ => self.skipped += 1,
        }
        Ok(())
    }

    fn add(
        &mut self,
        name: String,
        points: Vec<Pt>,
        closed: bool,
        t: Transform,
    ) -> Result<(), String> {
        let mut points: Vec<_> = points
            .into_iter()
            .map(|p| {
                let p = t.point(p);
                Pt {
                    x: p.x * self.scale,
                    y: -p.y * self.scale,
                    exact: p.exact,
                }
            })
            .collect();
        if points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e6 || p.y.abs() > 1e6)
        {
            return Err(
                "DXF has non-finite or excessively large coordinates. Check its units.".into(),
            );
        }
        points.dedup_by(|a, b| distance(*a, *b) < 1e-8);
        if points.len() < 2 {
            self.skipped += 1;
            return Ok(());
        }
        if closed || distance(points[0], *points.last().unwrap()) <= self.join_tolerance {
            contours::finish(
                &mut points,
                true,
                &name,
                &mut self.closed,
                &mut self.skipped,
            );
        } else {
            self.open.push(Contour { name, points });
        }
        Ok(())
    }
}
fn distance(a: Pt, b: Pt) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn arc(center: Pt, radius: f64, start: f64, sweep: f64, tolerance: f64) -> Result<Vec<Pt>, String> {
    if !radius.is_finite() || !start.is_finite() || !sweep.is_finite() || radius <= 0. {
        return Err("DXF contains an invalid arc.".into());
    }
    let angle = (2. * (1. - (tolerance / radius).min(1.)).acos()).max(1e-6);
    let steps = (sweep.abs() / angle).ceil().max(4.) as usize;
    if steps > 20_000 {
        return Err("DXF curve is too large or detailed. Check its units.".into());
    }
    Ok((0..=steps)
        .map(|i| {
            let a = start + sweep * i as f64 / steps as f64;
            Pt::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect())
}
fn polyline(vertices: &[(Pt, f64)], closed: bool, tolerance: f64) -> Result<Vec<Pt>, String> {
    if vertices.is_empty() {
        return Ok(vec![]);
    }
    let mut points = vec![vertices[0].0];
    let count = if closed {
        vertices.len()
    } else {
        vertices.len() - 1
    };
    for i in 0..count {
        let (a, bulge) = vertices[i];
        let b = vertices[(i + 1) % vertices.len()].0;
        if !bulge.is_finite() {
            return Err("DXF has an invalid polyline bulge.".into());
        }
        if bulge.abs() < 1e-10 || distance(a, b) < 1e-10 {
            if let Some(last) = points.last_mut() {
                last.exact = true;
            }
            points.push(Pt { exact: true, ..b });
            continue;
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let factor = (1. - bulge * bulge) / (4. * bulge);
        let center = Pt::new(
            (a.x + b.x) / 2. - dy * factor,
            (a.y + b.y) / 2. + dx * factor,
        );
        let sampled = arc(
            center,
            distance(center, a),
            (a.y - center.y).atan2(a.x - center.x),
            4. * bulge.atan(),
            tolerance,
        )?;
        points.extend(sampled.into_iter().skip(1));
        *points.last_mut().unwrap() = b;
        if points.len() > 20_000 {
            return Err("DXF polyline exceeds 20,000 points.".into());
        }
    }
    Ok(points)
}

pub fn export(project: &Project, layout: &Layout) -> Result<String, String> {
    let mut drawing = Drawing::new();
    drawing.header.version = AcadVersion::R2010;
    let unit = project.settings.dxf_export;
    let factor = 1. / unit.millimeters().unwrap_or(1.);
    drawing.header.default_drawing_units = match unit {
        CadUnit::Inches => Units::Inches,
        CadUnit::Centimeters => Units::Centimeters,
        CadUnit::Points | CadUnit::Picas => Units::Unitless,
        _ => Units::Millimeters,
    };
    let height = project.stock.height * layout.sheets.len() as f64
        + 20. * layout.sheets.len().saturating_sub(1) as f64;
    for (i, sheet) in layout.sheets.iter().enumerate() {
        let offset = i as f64 * (project.stock.height + 20.);
        if crate::cutting::shared_cuts(project) {
            for [a, b] in crate::cutting::segments(project, sheet) {
                let mut entity = Entity::new(EntityType::Line(Line::new(
                    dxf::Point::new(a.x * factor, (height - a.y - offset) * factor, 0.),
                    dxf::Point::new(b.x * factor, (height - b.y - offset) * factor, 0.),
                )));
                entity.common.layer = format!("SHEET_{:02}", i + 1);
                drawing.add_entity(entity);
            }
            continue;
        }
        for placement in sheet {
            let part = &project.parts[placement.part];
            for ring in std::iter::once(&part.points).chain(&part.holes) {
                let mut polyline = LwPolyline::default();
                polyline.set_is_closed(true);
                polyline.vertices = ring
                    .iter()
                    .map(|p| {
                        let p = transformed(*p, placement);
                        LwPolylineVertex {
                            x: p.x * factor,
                            y: (height - (p.y + offset)) * factor,
                            ..Default::default()
                        }
                    })
                    .collect();
                let mut entity = Entity::new(EntityType::LwPolyline(polyline));
                entity.common.layer = format!("SHEET_{:02}", i + 1);
                drawing.add_entity(entity);
            }
        }
    }
    let mut bytes = vec![];
    drawing
        .save(&mut bytes)
        .map_err(|e| format!("Cannot write DXF: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("Cannot encode DXF: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_overrides_and_export_units_preserve_physical_size() {
        let mut project = Project::demo();
        let layout = Layout {
            sheets: vec![vec![crate::model::PlacedPart {
                part: 0,
                instance: 0,
                x: 30.,
                y: 20.,
                rotation: 0.,
            }]],
            fitness: 0.,
            elapsed: 0.,
            pass: 1,
        };
        for unit in [
            CadUnit::Points,
            CadUnit::Picas,
            CadUnit::Inches,
            CadUnit::Millimeters,
            CadUnit::Centimeters,
        ] {
            project.settings.dxf_export = unit;
            let output = export(&project, &layout).unwrap();
            let settings = Settings {
                dxf_import: if matches!(unit, CadUnit::Points | CadUnit::Picas) {
                    unit
                } else {
                    CadUnit::Auto
                },
                ..Settings::default()
            };
            let imported = parse_with(output.as_bytes(), "units.dxf", &settings).unwrap();
            assert!((imported.parts[0].bounds().width - 120.).abs() < 1e-4);
            assert_eq!(imported.parts[0].holes.len(), 4);
            assert!(imported.parts[0].points.iter().all(|p| p.exact));
        }
        project.settings.dxf_export = CadUnit::Millimeters;
        let output = export(&project, &layout).unwrap();
        let settings = Settings {
            dxf_import: CadUnit::Inches,
            ..Settings::default()
        };
        let overridden = parse_with(output.as_bytes(), "units.dxf", &settings).unwrap();
        assert!((overridden.parts[0].bounds().width - 120. * 25.4).abs() < 1e-3);
    }
    use dxf::{
        entities::{Arc, Circle, Insert, Line},
        Block, Point,
    };

    #[test]
    fn imports_the_standalone_cad_fixture() {
        let imported = parse(
            include_bytes!("../../../examples/plate-with-hole.dxf"),
            "plate-with-hole.dxf",
        )
        .unwrap();
        assert_eq!(imported.parts.len(), 1);
        assert_eq!(imported.parts[0].holes.len(), 1);
        assert_eq!(imported.parts[0].bounds().width, 100.);
        assert_eq!(imported.parts[0].bounds().height, 60.);
        assert!(!imported.assumed_mm);
    }

    fn read(drawing: &Drawing, binary: bool) -> Import {
        let mut bytes = vec![];
        if binary {
            drawing.save_binary(&mut bytes).unwrap();
        } else {
            drawing.save(&mut bytes).unwrap();
        }
        parse(&bytes, "fixture.dxf").unwrap()
    }
    fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Entity {
        let mut p = LwPolyline::default();
        p.set_is_closed(true);
        p.vertices = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
            .into_iter()
            .map(|(x, y)| LwPolylineVertex {
                x,
                y,
                ..Default::default()
            })
            .collect();
        Entity::new(EntityType::LwPolyline(p))
    }
    fn drawing() -> Drawing {
        let mut d = Drawing::new();
        d.header.version = AcadVersion::R2010;
        d.header.default_drawing_units = Units::Millimeters;
        d
    }

    #[test]
    fn ascii_and_binary_dxf_preserve_inch_units_and_holes() {
        let mut d = drawing();
        d.header.default_drawing_units = Units::Inches;
        d.add_entity(rectangle(10., 20., 4., 2.));
        d.add_entity(Entity::new(EntityType::Circle(Circle {
            center: Point::new(12., 21., 0.),
            radius: 0.5,
            ..Default::default()
        })));
        for binary in [false, true] {
            let imported = read(&d, binary);
            assert_eq!(imported.parts.len(), 1);
            assert_eq!(imported.parts[0].holes.len(), 1);
            assert!((imported.parts[0].bounds().width - 101.6).abs() < 1e-8);
            assert!((imported.parts[0].bounds().height - 50.8).abs() < 1e-8);
            assert!(!imported.assumed_mm);
        }
    }
    #[test]
    fn unordered_lines_and_arcs_form_closed_contours_without_inventing_edges() {
        let mut d = drawing();
        for (a, b) in [
            ((0., -10.), (20., -10.)),
            ((0., 10.), (20., 10.)),
            ((0., -10.), (0., 10.)),
        ] {
            d.add_entity(Entity::new(EntityType::Line(Line {
                p1: Point::new(a.0, a.1, 0.),
                p2: Point::new(b.0, b.1, 0.),
                ..Default::default()
            })));
        }
        d.add_entity(Entity::new(EntityType::Arc(Arc {
            center: Point::new(20., 0., 0.),
            radius: 10.,
            start_angle: 270.,
            end_angle: 90.,
            ..Default::default()
        })));
        d.add_entity(Entity::new(EntityType::Line(Line {
            p1: Point::new(50., 50., 0.),
            p2: Point::new(60., 60., 0.),
            ..Default::default()
        })));
        let imported = read(&d, false);
        assert_eq!(imported.parts.len(), 1);
        assert_eq!(imported.skipped, 1);
        assert!((imported.parts[0].area() - (400. + 50. * std::f64::consts::PI)).abs() < 3.);
    }
    #[test]
    fn polyline_bulges_preserve_round_geometry_and_report_unitless_input() {
        let mut d = drawing();
        d.header.default_drawing_units = Units::Unitless;
        let mut p = LwPolyline::default();
        p.set_is_closed(true);
        p.vertices = vec![
            LwPolylineVertex {
                x: -10.,
                bulge: 1.,
                ..Default::default()
            },
            LwPolylineVertex {
                x: 10.,
                bulge: 1.,
                ..Default::default()
            },
        ];
        d.add_entity(Entity::new(EntityType::LwPolyline(p)));
        let imported = read(&d, false);
        assert!(imported.assumed_mm);
        assert!((imported.parts[0].area() - 100. * std::f64::consts::PI).abs() < 4.);
        assert!(imported.parts[0].points.len() > 16);
    }
    #[test]
    fn block_base_points_scaling_and_rotation_are_applied() {
        let mut d = drawing();
        d.add_block(Block {
            name: "plate".into(),
            base_point: Point::new(10., 20., 0.),
            entities: vec![rectangle(10., 20., 20., 10.)],
            ..Default::default()
        });
        d.add_entity(Entity::new(EntityType::Insert(Insert {
            name: "plate".into(),
            location: Point::new(100., 200., 0.),
            x_scale_factor: 2.,
            y_scale_factor: 3.,
            rotation: 90.,
            ..Default::default()
        })));
        let imported = read(&d, false);
        let b = imported.parts[0].bounds();
        assert!((b.width - 30.).abs() < 1e-8);
        assert!((b.height - 40.).abs() < 1e-8);
        assert!((imported.parts[0].area() - 1200.).abs() < 1e-8);
    }
    #[test]
    fn export_preserves_holes_rotation_units_and_sheet_layers() {
        let project = Project::demo();
        let layout = Layout {
            sheets: vec![
                vec![crate::model::PlacedPart {
                    part: 0,
                    instance: 0,
                    x: 90.,
                    y: 10.,
                    rotation: 90.,
                }],
                vec![crate::model::PlacedPart {
                    part: 4,
                    instance: 4000,
                    x: 10.,
                    y: 10.,
                    rotation: 0.,
                }],
            ],
            fitness: 1.,
            elapsed: 0.,
            pass: 1,
        };
        let source = export(&project, &layout).unwrap();
        let drawing = Drawing::load(&mut std::io::Cursor::new(source.as_bytes())).unwrap();
        assert_eq!(drawing.header.default_drawing_units, Units::Millimeters);
        assert!(drawing.entities().any(|e| e.common.layer == "SHEET_02"));
        let imported = parse(source.as_bytes(), "roundtrip.dxf").unwrap();
        assert_eq!(imported.parts.len(), 2);
        let plate = imported.parts.iter().find(|p| p.holes.len() == 4).unwrap();
        assert!((plate.bounds().width - 80.).abs() < 1e-6);
        assert!((plate.area() - project.parts[0].area()).abs() < 1e-6);
    }
}
