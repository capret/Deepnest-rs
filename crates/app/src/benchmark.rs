//! Headless, repeatable CPU measurements for the next optimizer stages.
use crate::{
    model::Project,
    nesting::{self, Event, SearchStats, Worker},
};
use std::path::PathBuf;

pub fn run(path: Option<PathBuf>) -> Result<(), String> {
    let project = if let Some(path) = path {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Project>(&source).map_err(|e| e.to_string())?
    } else {
        Project::demo()
    };
    project.validate()?;
    if project.settings.continuous {
        return Err("Benchmarks need a finite evaluation budget. Turn off continuous search in the project.".into());
    }
    let worker = Worker::start(project.clone());
    let mut best = None;
    let mut stats = SearchStats::default();
    let mut improvements = vec![];
    loop {
        match worker.events.recv().map_err(|e| e.to_string())? {
            Event::Improved(layout) => {
                improvements.push(serde_json::json!({ "evaluation": layout.pass, "elapsed_seconds": layout.elapsed, "placed": layout.placed(), "sheets": layout.sheets.len(), "utilization_percent": layout.utilization(&project) }));
                best = Some(layout);
            }
            Event::Measured(measured) => stats = measured,
            Event::Finished(result) => {
                result?;
                break;
            }
            Event::Progress { .. } => (),
        }
    }
    let best = best.ok_or("No layout produced")?;
    if !nesting::valid_layout(&project, &best) {
        return Err("Final layout failed geometric validation".into());
    }
    let output = serde_json::json!({
        "schema_version": 1, "engine": "cpu-diverse-ga", "project": project.name,
        "workers": project.settings.workers, "budget": project.attempts,
        "population": project.settings.population, "rotations": project.rotations,
        "valid": true, "placed": best.placed(), "requested": project.quantity(),
        "sheets": best.sheets.len(), "utilization_percent": best.utilization(&project),
        "fitness": best.fitness, "measurements": stats, "improvement_history": improvements,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?
    );
    Ok(())
}
