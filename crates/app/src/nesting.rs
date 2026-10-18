use crate::genetic::{Gene, Random};
use crate::model::{transformed, Layout, LayoutScore, Part, PlacedPart, Project};
use geo::{
    algorithm::buffer::{BufferStyle, LineJoin},
    Area, BooleanOps, Buffer, ConvexHull, Coord, LineString, Polygon,
};
use nest::{geom::polygon_area, Config, NfpCache, Poly, Pt};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    time::Instant,
};

pub enum Event {
    Progress { pass: u32, fraction: f64 },
    Improved(Layout),
    Measured(SearchStats),
    Finished(Result<(), String>),
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SearchStats {
    pub evaluations: u32,
    pub rejected: u32,
    pub improvements: u32,
    pub elapsed: f64,
    pub preparation_seconds: f64,
    pub validation_seconds: f64,
    pub first_layout_seconds: Option<f64>,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

pub struct Worker {
    pub events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Worker {
    pub fn start(project: Project) -> Self {
        let (sender, events) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| search(&project, &flag, &sender)))
                .unwrap_or_else(|_| Err("The geometry engine could not process these shapes. Try simpler closed outlines.".into()));
            let _ = sender.send(Event::Finished(outcome));
        });
        Self { events, cancel }
    }
}

fn ring(points: &[Pt]) -> LineString<f64> {
    LineString::new(points.iter().map(|p| Coord { x: p.x, y: p.y }).collect())
}

pub fn polygon(part: &Part) -> Polygon<f64> {
    Polygon::new(
        ring(&part.points),
        part.holes.iter().map(|points| ring(points)).collect(),
    )
}

fn from_polygon(polygon: &Polygon<f64>) -> Poly {
    fn points(ring: &LineString<f64>) -> Vec<Pt> {
        ring.0
            .iter()
            .take(ring.0.len().saturating_sub(1))
            .map(|p| Pt::new(p.x, p.y))
            .collect()
    }
    Poly {
        points: points(polygon.exterior()),
        children: polygon
            .interiors()
            .iter()
            .map(|ring| Poly::from_points(points(ring)))
            .collect(),
        ..Poly::default()
    }
}

fn expanded_parts(project: &Project) -> Result<Vec<Poly>, String> {
    project
        .parts
        .iter()
        .map(|part| {
            let shape = if project.settings.simplify {
                // A containing hull is conservative: nesting never changes the cut outline.
                polygon(part).convex_hull()
            } else {
                polygon(part)
            };
            if project.stock.gap == 0.0 {
                return Ok(if project.settings.simplify {
                    from_polygon(&shape)
                } else {
                    Poly {
                        points: part.points.clone(),
                        children: part.holes.iter().cloned().map(Poly::from_points).collect(),
                        ..Poly::default()
                    }
                });
            }
            // Offsetting each part by half the gap guarantees the requested spacing
            // while retaining concavities. The cut/export outline remains unchanged.
            let buffered = shape.buffer_with_style(
                BufferStyle::new(project.stock.gap / 2.0).line_join(LineJoin::Miter(2.0)),
            );
            if buffered.0.len() != 1 {
                return Err(format!(
                    "{} could not be offset. Repair its outline or reduce the part gap.",
                    part.name
                ));
            }
            Ok(from_polygon(&buffered.0[0]))
        })
        .collect()
}

fn search(
    project: &Project,
    cancel: &AtomicBool,
    sender: &SyncSender<Event>,
) -> Result<(), String> {
    project.validate()?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    if project.quantity() == 0 {
        return Err("Import parts or increase a quantity before nesting.".into());
    }
    let start = Instant::now();
    let expanded = expanded_parts(project)?;
    let mut stats = SearchStats {
        preparation_seconds: start.elapsed().as_secs_f64(),
        ..SearchStats::default()
    };
    let _ = sender.try_send(Event::Measured(stats.clone()));
    let cache = NfpCache::default();
    let settings = &project.settings;
    let config = Config {
        placement_type: settings.placement.key().into(),
        merge_lines: settings.merge_lines && project.stock.gap == 0. && !settings.simplify,
        scale: 25.4,
        curve_tolerance: settings.curve_tolerance,
        time_ratio: settings.time_ratio,
        rotations: project.rotations as f64,
        simplify: settings.simplify,
    };
    let mut genes: Vec<Gene> = project
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part, p)| {
            (0..p.quantity).map(move |copy| Gene {
                part,
                id: part as i64 * 1000 + copy as i64,
                rotation: 0,
            })
        })
        .collect();
    genes.sort_by(|a, b| {
        polygon_area(&expanded[b.part].points)
            .abs()
            .total_cmp(&polygon_area(&expanded[a.part].points).abs())
    });
    let mut random = Random::default();
    let mut population = random.population(
        genes,
        settings.population,
        settings.mutation,
        project.rotations,
    );
    let mut best: Option<Layout> = None;
    let mut pass = 0_u32;
    loop {
        let pending: Vec<usize> = population
            .iter()
            .enumerate()
            .filter(|(_, c)| c.score.is_none())
            .map(|(i, _)| i)
            .collect();
        for chunk in pending.chunks(settings.workers as usize) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let count = if settings.continuous {
                chunk.len()
            } else {
                chunk
                    .len()
                    .min(project.attempts.saturating_sub(pass) as usize)
            };
            if count == 0 {
                break;
            }
            // A small scoped batch bounds CPU usage; every job shares the NFP cache.
            let results = std::thread::scope(|scope| {
                let handles: Vec<_> = chunk[..count]
                    .iter()
                    .enumerate()
                    .map(|(offset, &index)| {
                        let genes = &population[index].genes;
                        let (expanded, cache, config) = (&expanded, &cache, &config);
                        let evaluation = pass + offset as u32 + 1;
                        scope.spawn(move || {
                            (
                                index,
                                evaluate(
                                    project,
                                    genes,
                                    expanded,
                                    config,
                                    cache,
                                    cancel,
                                    evaluation,
                                    start,
                                    (offset == 0).then_some(sender),
                                ),
                            )
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("Nesting worker panicked"))
                    .collect::<Vec<_>>()
            });
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            for (index, mut layout) in results {
                pass += 1;
                let validation_started = Instant::now();
                let valid = layout.placed() > 0 && valid_layout(project, &layout);
                stats.validation_seconds += validation_started.elapsed().as_secs_f64();
                stats.evaluations = pass;
                stats.rejected += u32::from(!valid);
                population[index].score = Some(if valid {
                    layout.score()
                } else {
                    LayoutScore::WORST
                });
                let improved = valid && best.as_ref().is_none_or(|old| layout.better_than(old));
                if improved {
                    // Timestamp the accepted solution, after validation and the
                    // parallel batch have finished, rather than its evaluation.
                    layout.elapsed = start.elapsed().as_secs_f64();
                    stats.improvements += 1;
                    stats
                        .first_layout_seconds
                        .get_or_insert_with(|| start.elapsed().as_secs_f64());
                    best = Some(layout.clone());
                    if sender.send(Event::Improved(layout)).is_err() {
                        return Ok(());
                    }
                }
                let fraction = if settings.continuous {
                    (pass % settings.population) as f64 / settings.population as f64
                } else {
                    pass as f64 / project.attempts as f64
                };
                let _ = sender.try_send(Event::Progress { pass, fraction });
            }
            stats.elapsed = start.elapsed().as_secs_f64();
            (stats.cache_hits, stats.cache_misses) = cache.statistics();
            let _ = sender.try_send(Event::Measured(stats.clone()));
        }
        if !settings.continuous && pass >= project.attempts {
            break;
        }
        random.generation(&mut population, settings.mutation, project.rotations);
    }
    stats.elapsed = start.elapsed().as_secs_f64();
    (stats.cache_hits, stats.cache_misses) = cache.statistics();
    let _ = sender.send(Event::Measured(stats));
    if best.is_none() {
        return Err("No valid layout found. Increase the stock size, reduce the gap, or simplify the outlines.".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    project: &Project,
    genes: &[Gene],
    expanded: &[Poly],
    config: &Config,
    cache: &NfpCache,
    cancel: &AtomicBool,
    pass: u32,
    start: Instant,
    sender: Option<&SyncSender<Event>>,
) -> Layout {
    let margin = project.stock.gap / 2.;
    let stock = vec![
        Pt::new(margin, margin),
        Pt::new(project.stock.width - margin, margin),
        Pt::new(project.stock.width - margin, project.stock.height - margin),
        Pt::new(margin, project.stock.height - margin),
    ];
    let parts = genes
        .iter()
        .map(|gene| {
            let mut poly = expanded[gene.part].clone();
            poly.source = gene.part as i64;
            poly.id = gene.id;
            poly.rotation = gene.rotation as f64 * 360. / project.rotations as f64;
            poly
        })
        .collect();
    let sheets = (0..project.stock.sheets)
        .map(|i| {
            let mut poly = Poly::from_points(stock.clone());
            poly.source = 1_000_000;
            poly.id = i as i64;
            poly
        })
        .collect();
    let mut last_progress = Instant::now();
    let result = nest::place_parts(
        sheets,
        parts,
        config,
        cache,
        |progress| {
            if progress >= 0. && last_progress.elapsed().as_millis() >= 50 {
                if let Some(sender) = sender {
                    let fraction = if project.settings.continuous {
                        ((pass - 1) % project.settings.population) as f64
                            / project.settings.population as f64
                    } else {
                        (pass as f64 - 1. + progress) / project.attempts as f64
                    };
                    let _ = sender.try_send(Event::Progress { pass, fraction });
                }
                last_progress = Instant::now();
            }
        },
        || cancel.load(Ordering::Relaxed),
    );
    Layout {
        sheets: result
            .placements
            .iter()
            .filter(|s| !s.sheetplacements.is_empty())
            .map(|s| {
                s.sheetplacements
                    .iter()
                    .map(|p| PlacedPart {
                        part: p.source as usize,
                        instance: p.id,
                        x: p.x,
                        y: p.y,
                        rotation: p.rotation,
                    })
                    .collect()
            })
            .collect(),
        fitness: result.fitness,
        elapsed: start.elapsed().as_secs_f64(),
        pass,
    }
}

/// Verify cut polygons before presenting/exporting an engine result. The legacy
/// engine uses approximate Minkowski geometry; invalid candidates are discarded.
pub fn valid_layout(project: &Project, layout: &Layout) -> bool {
    if layout.sheets.len() > project.stock.sheets as usize || !layout.fitness.is_finite() {
        return false;
    }
    let mut instances = std::collections::HashSet::new();
    let mut quantities = vec![0_u32; project.parts.len()];
    for sheet in &layout.sheets {
        let mut previous: Vec<Polygon<f64>> = vec![];
        for placement in sheet {
            let Some(part) = project.parts.get(placement.part) else {
                return false;
            };
            if !instances.insert(placement.instance) || !placement.rotation.is_finite() {
                return false;
            }
            quantities[placement.part] += 1;
            if quantities[placement.part] > part.quantity {
                return false;
            }
            let convert = |points: &[Pt]| {
                ring(
                    &points
                        .iter()
                        .map(|p| transformed(*p, placement))
                        .collect::<Vec<_>>(),
                )
            };
            let shape = Polygon::new(
                convert(&part.points),
                part.holes.iter().map(|points| convert(points)).collect(),
            );
            if shape.exterior().0.iter().any(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || p.x < -1e-4
                    || p.y < -1e-4
                    || p.x > project.stock.width + 1e-4
                    || p.y > project.stock.height + 1e-4
            }) {
                return false;
            }
            let padded = if project.stock.gap > 0.0 {
                shape.buffer(project.stock.gap / 2.0 - 1e-5)
            } else {
                geo::MultiPolygon(vec![shape.clone()])
            };
            for other in &previous {
                let overlap = padded
                    .intersection(&geo::MultiPolygon(vec![other.clone()]))
                    .unsigned_area();
                if overlap > 0.001 {
                    return false;
                }
            }
            previous.extend(padded.0);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square_project() -> Project {
        let mut project = Project {
            attempts: 1,
            ..Project::default()
        };
        project.stock.gap = 0.;
        project.parts = vec![Part {
            name: "Square".into(),
            points: [(0., 0.), (20., 0.), (20., 20.), (0., 20.)]
                .into_iter()
                .map(|(x, y)| Pt { x, y, exact: true })
                .collect(),
            holes: vec![],
            quantity: 2,
        }];
        project
    }
    fn completed(project: Project) -> (Layout, u32) {
        let worker = Worker::start(project);
        let (mut best, mut passes) = (None, 0);
        loop {
            match worker
                .events
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap()
            {
                Event::Improved(layout) => best = Some(layout),
                Event::Progress { pass, .. } => passes = pass,
                Event::Measured(_) => (),
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
            }
        }
        (best.unwrap(), passes)
    }
    #[test]
    fn placement_strategy_and_shared_cut_weight_reach_the_engine() {
        let mut project = square_project();
        project.settings.time_ratio = 0.;
        let (material, _) = completed(project.clone());
        project.settings.time_ratio = 1.;
        let (cutting, _) = completed(project.clone());
        assert_eq!(cutting.placed(), 2);
        assert!(cutting.fitness < material.fitness);
        project.settings.placement = crate::settings::Placement::Gravity;
        let (gravity, _) = completed(project);
        assert_ne!(gravity.fitness, cutting.fitness);
    }
    #[test]
    fn parallel_generations_respect_the_budget_and_keep_valid_results() {
        let mut project = square_project();
        project.attempts = 23;
        project.settings.population = 5;
        project.settings.workers = 1;
        let (single, count) = completed(project.clone());
        assert_eq!(count, 23);
        project.settings.workers = 4;
        let (parallel, count) = completed(project.clone());
        assert_eq!(count, 23);
        assert_eq!(parallel.placed(), single.placed());
        assert!((parallel.fitness - single.fitness).abs() < 1e-6);
        assert!(valid_layout(&project, &parallel));
    }
    #[test]
    fn continuous_search_goes_past_the_budget_and_stops_on_request() {
        let mut project = square_project();
        project.settings.continuous = true;
        project.settings.population = 3;
        let worker = Worker::start(project);
        let mut beyond_budget = false;
        loop {
            match worker
                .events
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap()
            {
                Event::Progress { pass, .. } if pass > 3 => {
                    beyond_budget = true;
                    worker.cancel.store(true, Ordering::Relaxed);
                }
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
                _ => (),
            }
        }
        assert!(beyond_budget);
    }
    #[test]
    fn rough_approximation_encloses_the_part_without_editing_its_cut_outline() {
        let mut project = Project::demo();
        project.stock.gap = 0.;
        let original = serde_json::to_string(&project.parts).unwrap();
        project.settings.simplify = true;
        let expanded = expanded_parts(&project).unwrap();
        assert!(expanded[1].points.len() < project.parts[1].points.len());
        assert!(polygon_area(&expanded[1].points).abs() > project.parts[1].area());
        assert_eq!(serde_json::to_string(&project.parts).unwrap(), original);
    }
    #[test]
    fn search_places_parts_with_clearance_and_reports_completion() {
        let project = Project {
            attempts: 2,
            parts: vec![Part {
                name: "square".into(),
                points: vec![
                    Pt::new(0., 0.),
                    Pt::new(20., 0.),
                    Pt::new(20., 20.),
                    Pt::new(0., 20.),
                ],
                holes: vec![],
                quantity: 4,
            }],
            ..Project::default()
        };
        let worker = Worker::start(project.clone());
        let mut best = None;
        loop {
            match worker
                .events
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap()
            {
                Event::Improved(layout) => best = Some(layout),
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
                _ => (),
            }
        }
        let best = best.unwrap();
        assert_eq!(best.placed(), 4);
        assert!(valid_layout(&project, &best));
        assert_eq!(best.sheets.len(), 1);
    }
    #[test]
    fn cancellation_does_not_publish_a_stale_layout() {
        let project = Project::demo();
        let (tx, rx) = mpsc::sync_channel(32);
        search(&project, &AtomicBool::new(true), &tx).unwrap();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn mixed_shape_demo_produces_a_valid_complete_layout() {
        let project = Project {
            attempts: 2,
            ..Project::demo()
        };
        let worker = Worker::start(project.clone());
        let mut best = None;
        loop {
            match worker
                .events
                .recv_timeout(std::time::Duration::from_secs(30))
                .unwrap()
            {
                Event::Improved(layout) => best = Some(layout),
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
                _ => (),
            }
        }
        let best = best.unwrap();
        assert_eq!(best.placed(), project.quantity());
        assert!(valid_layout(&project, &best));
    }
    #[test]
    fn identical_ring_collision_region_has_no_hole() {
        let project = Project::demo();
        let expanded = expanded_parts(&project).unwrap();
        let result = nest::nfp::nfp(&expanded[3], &expanded[3]);
        assert_eq!(result.len(), 1);
        assert!(result[0].children.is_empty());
    }

    #[test]
    fn measurements_cover_the_finished_budget_and_geometry_cache() {
        let mut project = square_project();
        project.attempts = 12;
        let worker = Worker::start(project.clone());
        let mut stats = SearchStats::default();
        let mut previous: Option<Layout> = None;
        loop {
            match worker
                .events
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap()
            {
                Event::Measured(next) => {
                    assert!(next.evaluations >= stats.evaluations);
                    stats = next;
                }
                Event::Improved(layout) => {
                    assert!(previous.as_ref().is_none_or(|old| layout.better_than(old)));
                    assert!(valid_layout(&project, &layout));
                    previous = Some(layout);
                }
                Event::Finished(result) => {
                    result.unwrap();
                    break;
                }
                _ => (),
            }
        }
        assert_eq!(stats.evaluations, 12);
        assert!(stats.improvements > 0);
        assert!(stats.rejected <= stats.evaluations);
        assert!(stats.first_layout_seconds.unwrap() <= stats.elapsed);
        assert!(stats.preparation_seconds + stats.validation_seconds <= stats.elapsed);
        assert!(stats.cache_hits > 0 && stats.cache_misses > 0);
    }

    #[test]
    fn validation_rejects_duplicate_instances_excess_copies_and_sheet_counts() {
        let project = square_project();
        let part = PlacedPart {
            part: 0,
            instance: 0,
            x: 0.,
            y: 0.,
            rotation: 0.,
        };
        let second = PlacedPart {
            instance: 1,
            x: 30.,
            ..part.clone()
        };
        let mut layout = Layout {
            sheets: vec![vec![part.clone(), second]],
            fitness: 1.,
            elapsed: 0.,
            pass: 0,
        };
        assert!(valid_layout(&project, &layout));
        layout.sheets[0][1].instance = 0;
        assert!(!valid_layout(&project, &layout));
        layout.sheets[0][1].instance = 1;
        layout.sheets[0].push(PlacedPart {
            instance: 2,
            x: 60.,
            ..part
        });
        assert!(!valid_layout(&project, &layout));
        layout.sheets = vec![vec![]; project.stock.sheets as usize + 1];
        assert!(!valid_layout(&project, &layout));
    }
    #[test]
    fn intersecting_or_outside_layouts_are_rejected() {
        let project = Project::demo();
        let part = PlacedPart {
            part: 0,
            instance: 0,
            x: 0.,
            y: 0.,
            rotation: 0.,
        };
        let mut layout = Layout {
            sheets: vec![vec![part.clone(), part]],
            fitness: 0.,
            elapsed: 0.,
            pass: 0,
        };
        assert!(!valid_layout(&project, &layout));
        layout.sheets[0].pop();
        layout.sheets[0][0].x = -10.0;
        assert!(!valid_layout(&project, &layout));
    }
}
