//! Workspace-only state. Run measurements and navigation never enter project files.
use crate::{
    model::{Layout, Project},
    nesting::SearchStats,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryTab {
    #[default]
    Parts,
    Stock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Setup,
    Insights,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub elapsed: f64,
    pub utilization: f64,
}

#[derive(Default)]
pub struct Run {
    pub stats: SearchStats,
    pub history: Vec<Sample>,
    started: Option<Instant>,
}

impl Run {
    pub fn begin(&mut self, baseline: Option<&Layout>, project: &Project) {
        *self = Self {
            started: Some(Instant::now()),
            ..Self::default()
        };
        if let Some(layout) = baseline {
            self.history.push(Sample {
                elapsed: 0.,
                utilization: layout.utilization(project),
            });
        }
    }

    pub fn tick(&mut self) {
        if let Some(started) = self.started {
            self.stats.elapsed = started.elapsed().as_secs_f64();
        }
    }

    pub fn stop(&mut self) {
        self.tick();
        self.started = None;
    }

    pub fn record(&mut self, layout: &Layout, project: &Project) {
        // Preserve the first observation and recent improvements during long runs.
        if self.history.len() == 128 {
            self.history.remove(1);
        }
        self.history.push(Sample {
            elapsed: layout.elapsed,
            utilization: layout.utilization(project),
        });
    }
}

pub fn duration(seconds: f64) -> String {
    if seconds > 0. && seconds < 0.1 {
        format!("{:.1}ms", seconds * 1000.)
    } else if seconds >= 60. {
        format!("{}m {:02}s", seconds as u64 / 60, seconds as u64 % 60)
    } else {
        format!("{seconds:.1}s")
    }
}
