//! Project settings, in physical millimeters internally, with editable drafts.
use crate::model::Project;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Millimeters,
    Inches,
}
impl Units {
    pub fn factor(self) -> f64 {
        if self == Self::Inches {
            25.4
        } else {
            1.
        }
    }
    pub fn symbol(self) -> &'static str {
        if self == Self::Inches {
            "in"
        } else {
            "mm"
        }
    }
    pub fn display(self, mm: f64) -> String {
        let s = format!("{:.4}", mm / self.factor());
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
    pub fn input(self, mm: f64) -> String {
        let s = format!("{:.10}", mm / self.factor());
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
impl std::fmt::Display for Units {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Millimeters => "Millimeters",
            Self::Inches => "Inches",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Placement {
    Gravity,
    #[default]
    BoundingBox,
    Compact,
}
impl Placement {
    pub fn key(self) -> &'static str {
        match self {
            Self::Gravity => "gravity",
            Self::BoundingBox => "box",
            Self::Compact => "convexhull",
        }
    }
}
impl std::fmt::Display for Placement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Gravity => "Gravity",
            Self::BoundingBox => "Bounding box",
            Self::Compact => "Compact",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CadUnit {
    #[default]
    Auto,
    Points,
    Picas,
    Inches,
    Millimeters,
    Centimeters,
}
impl CadUnit {
    pub fn millimeters(self) -> Option<f64> {
        match self {
            Self::Auto => None,
            Self::Points => Some(25.4 / 72.),
            Self::Picas => Some(25.4 / 6.),
            Self::Inches => Some(25.4),
            Self::Millimeters => Some(1.),
            Self::Centimeters => Some(10.),
        }
    }
}
impl std::fmt::Display for CadUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "From file (auto)",
            Self::Points => "Points (1/72 in)",
            Self::Picas => "Picas (1/6 in)",
            Self::Inches => "Inches",
            Self::Millimeters => "Millimeters",
            Self::Centimeters => "Centimeters",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub units: Units,
    pub placement: Placement,
    pub curve_tolerance: f64,
    pub endpoint_tolerance: f64,
    pub simplify: bool,
    pub workers: u32,
    pub svg_dpi: f64,
    pub dxf_import: CadUnit,
    pub dxf_export: CadUnit,
    pub merge_lines: bool,
    pub time_ratio: f64,
    pub population: u32,
    pub mutation: u32,
    pub continuous: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            units: Units::Millimeters,
            placement: Placement::BoundingBox,
            curve_tolerance: 0.1,
            endpoint_tolerance: 0.01,
            simplify: false,
            workers: 2,
            svg_dpi: 96.,
            dxf_import: CadUnit::Auto,
            dxf_export: CadUnit::Millimeters,
            merge_lines: true,
            time_ratio: 0.5,
            population: 10,
            mutation: 10,
            continuous: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value, min, max) in [
            ("Curve tolerance", self.curve_tolerance, 0.001, 10.),
            ("Endpoint tolerance", self.endpoint_tolerance, 0.00001, 5.),
            ("SVG scale", self.svg_dpi, 1., 9600.),
            ("Cutting-time weight", self.time_ratio, 0., 1.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "{name} must be between {min} and {max}. Distances are measured in mm."
                ));
            }
        }
        if !(1..=8).contains(&self.workers) {
            return Err("Use 1–8 CPU workers.".into());
        }
        if !(3..=512).contains(&self.population) {
            return Err("Use a population of 3–512 candidates.".into());
        }
        if !(1..=50).contains(&self.mutation) {
            return Err("Mutation rate must be 1–50%.".into());
        }
        if self.dxf_export == CadUnit::Auto {
            return Err("Choose an explicit DXF export unit.".into());
        }
        Ok(())
    }
    pub fn same_nesting(&self, other: &Self) -> bool {
        self.placement == other.placement
            && self.simplify == other.simplify
            && self.merge_lines == other.merge_lines
            && self.time_ratio == other.time_ratio
            && self.curve_tolerance == other.curve_tolerance
            && self.population == other.population
            && self.mutation == other.mutation
            && self.workers == other.workers
            && self.continuous == other.continuous
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Nesting,
    Geometry,
    Files,
    Cutting,
    Search,
}
impl Section {
    pub const ALL: [Self; 5] = [
        Self::Nesting,
        Self::Geometry,
        Self::Files,
        Self::Cutting,
        Self::Search,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Nesting => "Layout",
            Self::Geometry => "Geometry",
            Self::Files => "Import & export",
            Self::Cutting => "Laser cutting",
            Self::Search => "Search",
        }
    }
    pub fn help(self) -> Help {
        match self {
            Self::Nesting => Help::Placement,
            Self::Geometry => Help::Curves,
            Self::Files => Help::Svg,
            Self::Cutting => Help::Merge,
            Self::Search => Help::Population,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Help {
    Units,
    Gap,
    Rotations,
    Placement,
    Curves,
    Endpoints,
    Simplify,
    Workers,
    Svg,
    DxfImport,
    DxfExport,
    Merge,
    Time,
    Population,
    Mutation,
    Budget,
}
#[derive(Debug, Clone, Copy)]
pub enum Number {
    Gap,
    Rotations,
    Curves,
    Endpoints,
    Svg,
    Workers,
    Population,
    Mutation,
    Time,
    Attempts,
}
impl Number {
    pub fn help(self) -> Help {
        match self {
            Self::Gap => Help::Gap,
            Self::Rotations => Help::Rotations,
            Self::Curves => Help::Curves,
            Self::Endpoints => Help::Endpoints,
            Self::Svg => Help::Svg,
            Self::Workers => Help::Workers,
            Self::Population => Help::Population,
            Self::Mutation => Help::Mutation,
            Self::Time => Help::Time,
            Self::Attempts => Help::Budget,
        }
    }
}
#[derive(Debug, Clone)]
pub enum Action {
    Open,
    Close,
    Apply,
    Reset,
    Section(Section),
    Help(Help),
    Input(Number, String),
    Units(Units),
    Placement(Placement),
    Simplify(bool),
    DxfImport(CadUnit),
    DxfExport(CadUnit),
    Merge(bool),
    Continuous(bool),
}
pub struct Dialog {
    pub settings: Settings,
    pub inputs: [String; 10],
    pub section: Section,
    pub help: Help,
    pub error: Option<String>,
}
impl Dialog {
    pub fn new(project: &Project) -> Self {
        let s = &project.settings;
        let values = [
            project.stock.gap,
            project.rotations as f64,
            s.curve_tolerance,
            s.endpoint_tolerance,
            s.svg_dpi,
            s.workers as f64,
            s.population as f64,
            s.mutation as f64,
            s.time_ratio,
            project.attempts as f64,
        ];
        Self {
            settings: s.clone(),
            inputs: std::array::from_fn(|i| {
                if [0, 2, 3].contains(&i) {
                    s.units.input(values[i])
                } else {
                    values[i].to_string()
                }
            }),
            section: Section::Nesting,
            help: Help::Placement,
            error: None,
        }
    }
    pub fn change_units(&mut self, units: Units) -> Result<(), String> {
        let scale = self.settings.units.factor();
        let distances = [0, 2, 3].map(|i| self.inputs[i].parse::<f64>());
        if distances
            .iter()
            .any(|v| !v.as_ref().is_ok_and(|v| v.is_finite()))
        {
            return Err("Correct the distance fields before changing units.".into());
        }
        for (i, value) in [0, 2, 3].into_iter().zip(distances) {
            self.inputs[i] = units.input(value.unwrap() * scale);
        }
        self.settings.units = units;
        Ok(())
    }
    pub fn apply(&self, project: &mut Project) -> Result<(), String> {
        let values: Vec<f64> = self
            .inputs
            .iter()
            .map(|s| s.parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| "Enter a valid number in every setting.".to_string())?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err("Settings must be finite numbers.".into());
        }
        for i in [1, 5, 6, 7, 9] {
            if values[i].fract() != 0. || values[i] < 0. {
                return Err("Rotations, workers, population, mutation, and evaluations must be whole numbers.".into());
            }
        }
        let mut s = self.settings.clone();
        let factor = s.units.factor();
        s.curve_tolerance = values[2] * factor;
        s.endpoint_tolerance = values[3] * factor;
        s.svg_dpi = values[4];
        s.workers = values[5] as u32;
        s.population = values[6] as u32;
        s.mutation = values[7] as u32;
        s.time_ratio = values[8];
        let preserve = |next: f64, previous: f64| {
            if (next - previous).abs() < 1e-8 {
                previous
            } else {
                next
            }
        };
        s.curve_tolerance = preserve(s.curve_tolerance, project.settings.curve_tolerance);
        s.endpoint_tolerance = preserve(s.endpoint_tolerance, project.settings.endpoint_tolerance);
        project.settings = s;
        project.stock.gap = preserve(values[0] * factor, project.stock.gap);
        project.rotations = values[1] as u32;
        project.attempts = values[9] as u32;
        project.validate()
    }
}
