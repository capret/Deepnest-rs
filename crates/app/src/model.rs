use nest::{
    geom::{get_polygon_bounds, polygon_area, Bounds},
    Pt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub points: Vec<Pt>,
    #[serde(default)]
    pub holes: Vec<Vec<Pt>>,
    pub quantity: u32,
}

impl Part {
    pub fn bounds(&self) -> Bounds {
        get_polygon_bounds(&self.points).unwrap_or(Bounds {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        })
    }

    pub fn area(&self) -> f64 {
        polygon_area(&self.points).abs()
            - self
                .holes
                .iter()
                .map(|ring| polygon_area(ring).abs())
                .sum::<f64>()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Stock {
    pub width: f64,
    pub height: f64,
    pub sheets: u32,
    pub gap: f64,
}

impl Default for Stock {
    fn default() -> Self {
        Self {
            width: 600.0,
            height: 400.0,
            sheets: 3,
            gap: 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub parts: Vec<Part>,
    pub stock: Stock,
    pub rotations: u32,
    pub attempts: u32,
    pub settings: crate::settings::Settings,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: 1,
            name: "Untitled project".into(),
            parts: vec![],
            stock: Stock::default(),
            rotations: 4,
            attempts: 16,
            settings: crate::settings::Settings::default(),
        }
    }
}

impl Project {
    pub fn quantity(&self) -> usize {
        self.parts.iter().map(|part| part.quantity as usize).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("This project was saved by an unsupported version of Deepnest.".into());
        }
        let valid = |x: f64, min: f64, max: f64| x.is_finite() && (min..=max).contains(&x);
        if !valid(self.stock.width, 10.0, 100_000.0)
            || !valid(self.stock.height, 10.0, 100_000.0)
            || !valid(self.stock.gap, 0.0, 50.0)
            || self.stock.gap * 2.0 >= self.stock.width.min(self.stock.height)
            || !(1..=100).contains(&self.stock.sheets)
        {
            return Err("Use stock dimensions of 10–100,000 mm, 1–100 sheets, and a gap smaller than half the sheet.".into());
        }
        self.settings.validate()?;
        if !(1..=32).contains(&self.rotations) || !(1..=100_000).contains(&self.attempts) {
            return Err("Unsupported rotation count or search budget.".into());
        }
        if self.quantity() > 2000 || self.parts.len() > 500 {
            return Err(
                "This workspace supports up to 500 shapes and 2,000 copies per project.".into(),
            );
        }
        for part in &self.parts {
            if part.quantity > 200 || !valid(part.area(), 0.000001, 1e12) {
                return Err(format!(
                    "{} has an invalid area or quantity (maximum 200).",
                    part.name
                ));
            }
            for ring in std::iter::once(&part.points).chain(&part.holes) {
                if !(3..=20_000).contains(&ring.len())
                    || ring
                        .iter()
                        .any(|p| !valid(p.x, -1e6, 1e6) || !valid(p.y, -1e6, 1e6))
                {
                    return Err(format!(
                        "{} contains invalid polygon coordinates.",
                        part.name
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn demo() -> Self {
        let polygon = |name: &str, coords: &[(f64, f64)], quantity| Part {
            name: name.into(),
            points: coords
                .iter()
                .map(|&(x, y)| Pt { x, y, exact: true })
                .collect(),
            holes: vec![],
            quantity,
        };
        let circle = |radius: f64, center: (f64, f64)| {
            (0..40)
                .map(|i| {
                    let angle = i as f64 * std::f64::consts::TAU / 40.0;
                    Pt::new(
                        center.0 + radius * angle.cos(),
                        center.1 + radius * angle.sin(),
                    )
                })
                .collect()
        };
        let mut plate = polygon(
            "Mounting plate",
            &[
                (8., 0.),
                (112., 0.),
                (120., 8.),
                (120., 72.),
                (112., 80.),
                (8., 80.),
                (0., 72.),
                (0., 8.),
            ],
            4,
        );
        plate.holes = vec![
            circle(5.0, (16.0, 16.0)),
            circle(5.0, (104.0, 16.0)),
            circle(5.0, (16.0, 64.0)),
            circle(5.0, (104.0, 64.0)),
        ];
        Self {
            name: "Assembly kit".into(),
            stock: Stock {
                width: 480.0,
                height: 320.0,
                sheets: 3,
                gap: 3.0,
            },
            parts: vec![
                plate,
                polygon(
                    "Angle bracket",
                    &[
                        (0., 0.),
                        (100., 0.),
                        (100., 30.),
                        (35., 30.),
                        (35., 90.),
                        (0., 90.),
                    ],
                    4,
                ),
                polygon("Corner gusset", &[(0., 0.), (96., 0.), (0., 84.)], 4),
                Part {
                    name: "Spacer ring".into(),
                    points: circle(32.0, (32., 32.)),
                    holes: vec![circle(16.0, (32., 32.))],
                    quantity: 6,
                },
                polygon(
                    "Connector tab",
                    &[
                        (10., 0.),
                        (34., 0.),
                        (44., 10.),
                        (44., 82.),
                        (34., 92.),
                        (10., 92.),
                        (0., 82.),
                        (0., 10.),
                    ],
                    4,
                ),
            ],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacedPart {
    pub part: usize,
    pub instance: i64,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub sheets: Vec<Vec<PlacedPart>>,
    pub fitness: f64,
    pub elapsed: f64,
    pub pass: u32,
}

impl Layout {
    pub fn placed(&self) -> usize {
        self.sheets.iter().map(Vec::len).sum()
    }
    pub fn utilization(&self, project: &Project) -> f64 {
        if self.sheets.is_empty() {
            return 0.0;
        }
        self.sheets
            .iter()
            .flatten()
            .map(|p| project.parts[p.part].area())
            .sum::<f64>()
            / (self.sheets.len() as f64 * project.stock.width * project.stock.height)
            * 100.0
    }
}

pub fn transformed(point: Pt, placement: &PlacedPart) -> Pt {
    let (sin, cos) = placement.rotation.to_radians().sin_cos();
    Pt::new(
        point.x * cos - point.y * sin + placement.x,
        point.x * sin + point.y * cos + placement.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_and_project_round_trip_are_valid() {
        let project = Project::demo();
        project.validate().unwrap();
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.quantity(), 22);
        assert!(!json.contains("motion"));
    }
    #[test]
    fn invalid_stock_and_geometry_are_rejected() {
        let mut project = Project::demo();
        project.stock.width = f64::NAN;
        assert!(project.validate().is_err());
        project.stock.width = 480.0;
        project.parts[0].points[0].x = f64::INFINITY;
        assert!(project.validate().is_err());
    }

    #[test]
    fn legacy_motion_settings_are_ignored_on_open() {
        let mut json = serde_json::to_value(Project::demo()).unwrap();
        json["motion"] = serde_json::json!({"mode":"Physics","duration":9.0,"bounce":0.8});
        let project: Project = serde_json::from_value(json).unwrap();
        project.validate().unwrap();
        assert!(serde_json::to_value(project)
            .unwrap()
            .get("motion")
            .is_none());
    }
}
