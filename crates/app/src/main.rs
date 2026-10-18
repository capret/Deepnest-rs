#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod benchmark;
mod camera;
mod canvas;
mod contours;
mod cutting;
mod dxf_io;
mod genetic;
mod icons;
mod model;
mod motion;
mod nesting;
mod settings;
mod settings_view;
mod studio;
mod style;
mod svg_io;
mod tokens;
mod tooltip;
mod view;
mod widgets;
mod windowing;

use deepnest_motion::Value;
use iced::{
    event, keyboard, window, Element, Event, Point, Size, Subscription, Task, Theme, Vector,
};
use model::{Layout, Part, Project};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

pub fn main() -> iced::Result {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_some_and(|arg| arg == "--benchmark") {
        if let Err(error) = benchmark::run(args.next().map(PathBuf::from)) {
            eprintln!("Benchmark failed: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let window_settings = window::Settings {
        size: Size::new(1360., 860.),
        min_size: Some(Size::new(500., 660.)),
        decorations: !cfg!(target_os = "windows"),
        platform_specific: windowing::platform_settings(),
        ..Default::default()
    };
    iced::application(App::new, App::update, App::view)
        .title("Deepnest — Nesting workspace")
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window_settings)
        .settings(iced::Settings {
            default_text_size: tokens::typography::BODY.into(),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StockField {
    Width,
    Height,
    Sheets,
    Gap,
}

#[derive(Debug, Clone)]
struct ImportBatch {
    parts: Vec<Part>,
    skipped: usize,
    files: usize,
    assumed_mm: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Svg,
    Dxf,
}
impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Svg => "SVG",
            Self::Dxf => "DXF",
        })
    }
}

#[derive(Debug, Clone)]
enum Message {
    Window(WindowAction),
    RefreshWindow(window::Id),
    WindowMaximized(bool),
    NativeTitlebarInstalled(bool),
    Frame(Instant),
    Poll,
    Import,
    Imported(Result<Option<ImportBatch>, String>),
    Dropped(PathBuf),
    Open,
    Opened(Result<Option<Project>, String>),
    Save,
    Saved(u64, Result<Option<String>, String>),
    Export,
    Exported(Result<Option<String>, String>),
    ExportFormat(ExportFormat),
    New,
    Demo,
    Undo,
    SelectPart(usize),
    LibraryTab(studio::LibraryTab),
    InspectorTab(studio::InspectorTab),
    FilterParts(String),
    Continuous(bool),
    Placement(settings::Placement),
    Quantity(usize, i32),
    RemovePart(usize),
    StockInput(StockField, String),
    CommitStock,
    Rotations(u32),
    Attempts(u32),
    Nest,
    Stop,
    SelectSheet(usize),
    Zoom(f64),
    ZoomAt(f64, Point, Size),
    Fit,
    StartPan,
    Pan(Vector),
    ToggleHelp,
    ToggleGrid,
    ToggleProjectMenu,
    HintEnter(&'static str, iced::Rectangle),
    HintLeave(&'static str),
    Resized(Size),
    Unfocused,
    FocusNext(bool),
    Settings(settings::Action),
}

#[derive(Debug, Clone, Copy)]
enum WindowAction {
    Drag,
    Minimize,
    Maximize,
    Close,
    SystemMenu,
}

struct Pose {
    x: Value,
    y: Value,
    rotation: Value,
}
impl Pose {
    fn tick(&mut self, now: f64) {
        self.x.tick(now);
        self.y.tick(now);
        self.rotation.tick(now);
    }
    fn active(&self) -> bool {
        self.x.is_active() || self.y.is_active() || self.rotation.is_active()
    }
}

struct App {
    project: Project,
    layout: Option<Layout>,
    selected: Option<usize>,
    sheet: usize,
    worker: Option<nesting::Worker>,
    progress: f64,
    nest_progress: Value,
    run: studio::Run,
    library_tab: studio::LibraryTab,
    inspector_tab: studio::InspectorTab,
    part_filter: String,
    pass: u32,
    poses: HashMap<i64, Pose>,
    zoom: Value,
    pan: Vector,
    zoom_anchor: Option<camera::Anchor>,
    epoch: Instant,
    show_project_menu: bool,
    show_grid: bool,
    show_help: bool,
    menu_progress: Value,
    tooltip: tooltip::Tooltip,
    window_size: Size,
    window_maximized: bool,
    status: String,
    error: bool,
    busy_file: bool,
    dirty: bool,
    revision: u64,
    history: Vec<Project>,
    stock_inputs: [String; 4],
    export_format: ExportFormat,
    settings_dialog: Option<settings::Dialog>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut app = Self {
            project: Project::demo(),
            layout: None,
            selected: Some(0),
            sheet: 0,
            worker: None,
            progress: 0.0,
            nest_progress: Value::new(0.72),
            run: studio::Run::default(),
            library_tab: studio::LibraryTab::Parts,
            inspector_tab: studio::InspectorTab::Setup,
            part_filter: String::new(),
            pass: 0,
            poses: HashMap::new(),
            zoom: Value::new(1.0),
            pan: Vector::ZERO,
            zoom_anchor: None,
            epoch: Instant::now(),
            show_project_menu: false,
            show_grid: true,
            show_help: false,
            menu_progress: Value::new(0.),
            tooltip: tooltip::Tooltip::default(),
            window_size: Size::new(1360., 860.),
            window_maximized: false,
            status: "Sample project · Ready to nest".into(),
            error: false,
            busy_file: false,
            dirty: false,
            revision: 0,
            history: vec![],
            stock_inputs: Default::default(),
            export_format: ExportFormat::Svg,
            settings_dialog: None,
        };
        app.sync_stock();
        (app, window::oldest().and_then(refresh_window))
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
    fn theme(&self) -> Theme {
        Theme::custom(
            "Deepnest Obsidian",
            iced::theme::Palette {
                background: tokens::color::BACKGROUND,
                text: tokens::color::FOREGROUND,
                primary: tokens::color::PRIMARY,
                success: tokens::color::SUCCESS,
                warning: tokens::color::WARNING,
                danger: tokens::color::DESTRUCTIVE,
            },
        )
    }
    fn view(&self) -> Element<'_, Message> {
        view::workspace(self)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![event::listen_with(|event, status, id| match event {
            Event::Window(
                window::Event::Opened { .. } | window::Event::Rescaled(_) | window::Event::Focused,
            ) => Some(Message::RefreshWindow(id)),
            Event::Window(window::Event::FileDropped(path)) => Some(Message::Dropped(path)),
            Event::Window(window::Event::Resized(size)) => Some(Message::Resized(size)),
            Event::Window(window::Event::Unfocused) => Some(Message::Unfocused),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                repeat: false,
                ..
            }) if status == event::Status::Ignored => match key.as_ref() {
                keyboard::Key::Character("o") if modifiers.command() => Some(Message::Open),
                keyboard::Key::Character(",") if modifiers.command() => {
                    Some(Message::Settings(settings::Action::Open))
                }
                keyboard::Key::Character("i") if modifiers.command() => Some(Message::Import),
                keyboard::Key::Character("s") if modifiers.command() => Some(Message::Save),
                keyboard::Key::Character("e") if modifiers.command() => Some(Message::Export),
                keyboard::Key::Character("z") if modifiers.command() => Some(Message::Undo),
                keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Stop),
                keyboard::Key::Named(keyboard::key::Named::Enter) if modifiers.command() => {
                    Some(Message::Nest)
                }
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    Some(Message::FocusNext(modifiers.shift()))
                }
                _ => None,
            },
            _ => None,
        })];
        if self.zoom.is_active()
            || self.nest_progress.is_active()
            || self.poses.values().any(Pose::active)
            || self.menu_progress.is_active()
            || self.tooltip.active()
        {
            subscriptions.push(window::frames().map(Message::Frame));
        }
        if self.worker.is_some() {
            subscriptions.push(iced::time::every(Duration::from_millis(50)).map(|_| Message::Poll));
        }
        Subscription::batch(subscriptions)
    }

    fn sync_stock(&mut self) {
        let s = &self.project.stock;
        let units = self.project.settings.units;
        self.stock_inputs = [
            units.input(s.width),
            units.input(s.height),
            s.sheets.to_string(),
            units.input(s.gap),
        ];
    }
    fn stock_pending(&self) -> bool {
        let s = &self.project.stock;
        self.stock_inputs
            .iter()
            .zip([s.width, s.height, s.sheets as f64, s.gap])
            .enumerate()
            .any(|(i, (input, value))| {
                input.parse::<f64>().ok().is_none_or(|v| {
                    let factor = if i == 2 {
                        1.
                    } else {
                        self.project.settings.units.factor()
                    };
                    !v.is_finite() || (v * factor - value).abs() > 1e-8
                })
            })
    }
    fn notice(&mut self, message: impl Into<String>, error: bool) {
        self.status = message.into();
        self.error = error;
    }
    fn checkpoint(&mut self) {
        self.history.push(self.project.clone());
        if self.history.len() > 30 {
            self.history.remove(0);
        }
        self.dirty = true;
        self.revision += 1;
    }
    fn invalidate_layout(&mut self) {
        self.stop_zoom();
        self.worker = None;
        self.layout = None;
        self.poses.clear();
        self.sheet = 0;
        self.progress = 0.0;
        self.pass = 0;
        self.run = studio::Run::default();
    }
    fn stop_zoom(&mut self) {
        self.zoom.jump(self.zoom.current);
        self.zoom_anchor = None;
    }
    fn commit_stock(&mut self) -> bool {
        let parsed: Option<Vec<f64>> = self
            .stock_inputs
            .iter()
            .map(|s| s.parse::<f64>().ok())
            .collect();
        let Some(mut values) = parsed else {
            self.notice("Enter a valid number for each sheet setting.", true);
            return false;
        };
        for i in [0, 1, 3] {
            values[i] *= self.project.settings.units.factor();
        }
        let mut project = self.project.clone();
        project.stock.width = values[0];
        project.stock.height = values[1];
        project.stock.sheets = values[2] as u32;
        project.stock.gap = values[3];
        if !values[2].is_finite() || values[2].fract() != 0.0 || !(1.0..=100.0).contains(&values[2])
        {
            self.notice("Sheet count must be a whole number from 1 to 100.", true);
            return false;
        }
        if let Err(error) = project.validate() {
            self.notice(error, true);
            return false;
        }
        let previous = &self.project.stock;
        let changed = (previous.width - values[0]).abs() > 1e-8
            || (previous.height - values[1]).abs() > 1e-8
            || previous.sheets != values[2] as u32
            || (previous.gap - values[3]).abs() > 1e-8;
        if changed {
            self.checkpoint();
            self.project.stock = project.stock;
            self.invalidate_layout();
            self.notice(
                "Sheet settings updated. Run Nest to calculate a new layout.",
                false,
            );
        }
        self.sync_stock();
        true
    }

    fn accept_layout(&mut self, layout: Layout) {
        // A refinement run may find a worse initial result. Keep the incumbent
        // visible until a verified candidate actually improves the objective.
        if self
            .layout
            .as_ref()
            .is_some_and(|old| !layout.better_than(old))
        {
            return;
        }
        self.run.record(&layout, &self.project);
        let now = self.now();
        let origins = canvas::preview_placements(&self.project);
        for part in layout.sheets.iter().flatten() {
            let origin = origins.iter().find(|p| p.part == part.part);
            let pose = self.poses.entry(part.instance).or_insert_with(|| Pose {
                x: Value::new(origin.map_or(part.x, |p| p.x)),
                y: Value::new(origin.map_or(part.y, |p| p.y)),
                rotation: Value::new(0.),
            });
            pose.x.retarget(part.x, now, motion::layout());
            pose.y.retarget(part.y, now, motion::layout());
            // Rotate along the shortest path, avoiding full turns on a new pass.
            let rotation = pose.rotation.current
                + (part.rotation - pose.rotation.current + 180.).rem_euclid(360.)
                - 180.;
            pose.rotation.retarget(rotation, now, motion::layout());
        }
        self.sheet = self.sheet.min(layout.sheets.len().saturating_sub(1));
        self.layout = Some(layout);
    }

    fn poll_worker(&mut self) {
        self.run.tick();
        let events: Vec<_> = self
            .worker
            .as_ref()
            .map(|worker| worker.events.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                nesting::Event::Progress { pass, fraction } => {
                    self.pass = pass;
                    self.progress = fraction;
                }
                nesting::Event::Improved(layout) => self.accept_layout(layout),
                nesting::Event::Measured(stats) => self.run.stats = stats,
                nesting::Event::Finished(result) => {
                    self.run.stop();
                    self.worker = None;
                    match result {
                        Ok(()) => {
                            self.progress = 1.0;
                            if let Some(layout) = &self.layout {
                                let missing =
                                    self.project.quantity().saturating_sub(layout.placed());
                                let message = if missing > 0 {
                                    format!("{} parts placed · {missing} unplaced. Add sheets or enlarge the stock.",layout.placed())
                                } else {
                                    format!(
                                        "All {} parts placed across {} sheet{}. Ready to export.",
                                        layout.placed(),
                                        layout.sheets.len(),
                                        if layout.sheets.len() == 1 { "" } else { "s" }
                                    )
                                };
                                self.notice(message, missing > 0);
                            }
                        }
                        Err(error) => self.notice(error, true),
                    }
                }
            }
        }
    }

    fn settings_action(&mut self, action: settings::Action) {
        use settings::Action;
        match action {
            Action::Open if !self.busy_file => {
                if self.commit_stock() {
                    self.settings_dialog = Some(settings::Dialog::new(&self.project));
                    self.show_project_menu = false;
                    self.menu_progress.jump(0.);
                }
            }
            Action::Close => self.settings_dialog = None,
            Action::Apply => {
                if let Some(dialog) = &self.settings_dialog {
                    let mut project = self.project.clone();
                    if let Err(error) = dialog.apply(&mut project) {
                        self.settings_dialog.as_mut().unwrap().error = Some(error);
                        return;
                    }
                    let changed = project.settings != self.project.settings
                        || project.rotations != self.project.rotations
                        || project.attempts != self.project.attempts
                        || project.stock.gap != self.project.stock.gap;
                    if changed {
                        let rerun = !project.settings.same_nesting(&self.project.settings)
                            || project.rotations != self.project.rotations
                            || project.stock.gap != self.project.stock.gap
                            || project.attempts != self.project.attempts;
                        self.checkpoint();
                        if rerun {
                            self.invalidate_layout();
                        }
                        self.project = project;
                        self.sync_stock();
                        self.notice(
                            "Settings saved. Import settings apply to the next import.",
                            false,
                        );
                    }
                    self.settings_dialog = None;
                }
            }
            Action::Reset => {
                let section = self
                    .settings_dialog
                    .as_ref()
                    .map_or(settings::Section::Nesting, |d| d.section);
                let mut dialog = settings::Dialog::new(&Project::default());
                dialog.section = section;
                dialog.help = section.help();
                self.settings_dialog = Some(dialog);
            }
            action => {
                if let Some(dialog) = &mut self.settings_dialog {
                    if !matches!(action, Action::Help(_) | Action::Section(_)) {
                        dialog.error = None;
                    }
                    match action {
                        Action::Section(section) => {
                            dialog.section = section;
                            dialog.help = section.help();
                        }
                        Action::Help(help) => dialog.help = help,
                        Action::Input(field, value) => {
                            dialog.inputs[field as usize] = value;
                            dialog.help = field.help();
                        }
                        Action::Units(units) => {
                            dialog.error = dialog.change_units(units).err();
                            dialog.help = settings::Help::Units;
                        }
                        Action::Placement(value) => {
                            dialog.settings.placement = value;
                            dialog.help = settings::Help::Placement;
                        }
                        Action::Simplify(value) => {
                            dialog.settings.simplify = value;
                            dialog.help = settings::Help::Simplify;
                        }
                        Action::DxfImport(value) => {
                            dialog.settings.dxf_import = value;
                            dialog.help = settings::Help::DxfImport;
                        }
                        Action::DxfExport(value) => {
                            dialog.settings.dxf_export = value;
                            dialog.help = settings::Help::DxfExport;
                        }
                        Action::Merge(value) => {
                            dialog.settings.merge_lines = value;
                            dialog.help = settings::Help::Merge;
                        }
                        Action::Continuous(value) => {
                            dialog.settings.continuous = value;
                            dialog.help = settings::Help::Budget;
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Window(action) = message {
            return window::latest().and_then(move |id| match action {
                WindowAction::Drag => window::drag(id),
                WindowAction::Minimize => window::minimize(id, true),
                WindowAction::Maximize => window::toggle_maximize(id),
                WindowAction::Close => window::close(id),
                WindowAction::SystemMenu => window::show_system_menu(id),
            });
        }
        match message {
            Message::RefreshWindow(id) => return refresh_window(id),
            Message::WindowMaximized(maximized) => {
                self.window_maximized = maximized;
                return Task::none();
            }
            Message::NativeTitlebarInstalled(installed) => {
                if !installed {
                    eprintln!("Could not install the native window frame integration");
                }
                return Task::none();
            }
            _ => (),
        }
        let now = self.now();
        if self.settings_dialog.is_some()
            && !matches!(
                message,
                Message::Settings(_)
                    | Message::Frame(_)
                    | Message::Poll
                    | Message::Stop
                    | Message::Resized(_)
                    | Message::Unfocused
                    | Message::FocusNext(_)
                    | Message::HintEnter(..)
                    | Message::HintLeave(_)
                    | Message::Imported(_)
                    | Message::Opened(_)
                    | Message::Saved(..)
                    | Message::Exported(_)
            )
        {
            return Task::none();
        }
        if !matches!(
            message,
            Message::Frame(_) | Message::Poll | Message::HintEnter(..) | Message::HintLeave(_)
        ) {
            self.tooltip.dismiss(now);
        }
        if matches!(
            message,
            Message::New | Message::Demo | Message::Open | Message::Save | Message::Import
        ) {
            self.show_project_menu = false;
            self.menu_progress.retarget(0., now, motion::panel());
        }
        match message {
            Message::Settings(action) => self.settings_action(action),
            Message::Frame(at) => {
                let now = at.saturating_duration_since(self.epoch).as_secs_f64();
                self.zoom.tick(now);
                if let Some(anchor) = self.zoom_anchor {
                    self.pan = anchor.pan(&self.project.stock, self.zoom.current);
                    if !self.zoom.is_active() {
                        self.zoom_anchor = None;
                    }
                }
                self.menu_progress.tick(now);
                self.nest_progress.tick(now);
                self.tooltip.tick(now);
                for pose in self.poses.values_mut() {
                    pose.tick(now);
                }
            }
            Message::Poll => self.poll_worker(),
            Message::Import if !self.busy_file => {
                self.busy_file = true;
                return Task::perform(
                    import_dialog(self.project.settings.clone()),
                    Message::Imported,
                );
            }
            Message::Dropped(path) if !self.busy_file => {
                self.busy_file = true;
                if path.extension().is_some_and(|e| e == "deepnest") {
                    return Task::perform(
                        async move { read_project(path).await.map(Some) },
                        Message::Opened,
                    );
                }
                return Task::perform(
                    read_vectors(vec![path], self.project.settings.clone()),
                    Message::Imported,
                );
            }
            Message::Imported(result) => {
                self.busy_file = false;
                match result {
                    Ok(Some(batch)) => {
                        let mut candidate = self.project.clone();
                        let count = batch.parts.len();
                        candidate.parts.extend(batch.parts);
                        if let Err(error) = candidate.validate() {
                            self.notice(error, true);
                        } else {
                            self.checkpoint();
                            self.invalidate_layout();
                            self.selected = Some(self.project.parts.len());
                            self.project.parts = candidate.parts;
                            self.part_filter.clear();
                            self.library_tab = studio::LibraryTab::Parts;
                            self.notice(
                                format!(
                                    "{}Imported {count} shapes from {} file(s).{}",
                                    if batch.assumed_mm > 0 {
                                        "Unitless DXF: using mm. "
                                    } else {
                                        ""
                                    },
                                    batch.files,
                                    if batch.skipped > 0 {
                                        format!(
                                            " Skipped {} open contours or unsupported objects.",
                                            batch.skipped
                                        )
                                    } else {
                                        String::new()
                                    }
                                ),
                                false,
                            );
                        }
                    }
                    Ok(None) => (),
                    Err(error) => self.notice(error, true),
                }
            }
            Message::Open if !self.busy_file => {
                self.busy_file = true;
                return Task::perform(
                    async {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("Deepnest project", &["deepnest"])
                            .pick_file()
                            .await
                        else {
                            return Ok(None);
                        };
                        read_project(file.path().to_owned()).await.map(Some)
                    },
                    Message::Opened,
                );
            }
            Message::Opened(result) => {
                self.busy_file = false;
                match result {
                    Ok(Some(project)) => {
                        self.checkpoint();
                        self.invalidate_layout();
                        self.project = project;
                        self.part_filter.clear();
                        self.selected = if self.project.parts.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.dirty = false;
                        self.sync_stock();
                        self.zoom.jump(1.);
                        self.pan = Vector::ZERO;
                        self.notice(
                            "Project opened. Run Nest to calculate its cutting layout.",
                            false,
                        );
                    }
                    Ok(None) => (),
                    Err(error) => self.notice(error, true),
                }
            }
            Message::Save if !self.busy_file => {
                if !self.commit_stock() {
                    return Task::none();
                }
                let contents = match serde_json::to_string_pretty(&self.project) {
                    Ok(s) => s,
                    Err(e) => {
                        self.notice(e.to_string(), true);
                        return Task::none();
                    }
                };
                self.busy_file = true;
                let revision = self.revision;
                return Task::perform(
                    save_dialog(
                        "assembly.deepnest",
                        "Deepnest project",
                        "deepnest",
                        contents,
                    ),
                    move |result| Message::Saved(revision, result),
                );
            }
            Message::Saved(revision, result) => {
                self.busy_file = false;
                match result {
                    Ok(Some(path)) => {
                        if revision == self.revision {
                            self.dirty = false;
                        }
                        self.notice(format!("Project saved to {path}"), false);
                    }
                    Ok(None) => (),
                    Err(error) => self.notice(error, true),
                }
            }
            Message::Export if !self.busy_file && self.worker.is_none() => {
                if !self.commit_stock() {
                    return Task::none();
                }
                if let Some(layout) = &self.layout {
                    if !nesting::valid_layout(&self.project, layout) {
                        self.notice("This layout did not pass validation. Run nesting again before exporting.", true);
                        return Task::none();
                    }
                    let (name, filter, extension, contents) = match self.export_format {
                        ExportFormat::Svg => (
                            "cutting-layout.svg",
                            "SVG cutting layout",
                            "svg",
                            svg_io::export(&self.project, layout),
                        ),
                        ExportFormat::Dxf => match dxf_io::export(&self.project, layout) {
                            Ok(contents) => {
                                ("cutting-layout.dxf", "DXF cutting layout", "dxf", contents)
                            }
                            Err(error) => {
                                self.notice(error, true);
                                return Task::none();
                            }
                        },
                    };
                    self.busy_file = true;
                    return Task::perform(
                        save_dialog(name, filter, extension, contents),
                        Message::Exported,
                    );
                } else {
                    self.notice("Run Nest before exporting a cutting layout.", true);
                }
            }
            Message::Exported(result) => {
                self.busy_file = false;
                match result {
                    Ok(Some(path)) => {
                        self.notice(format!("Cutting layout exported to {path}"), false)
                    }
                    Ok(None) => (),
                    Err(error) => self.notice(error, true),
                }
            }
            Message::ExportFormat(format) => self.export_format = format,
            Message::New | Message::Demo => {
                self.checkpoint();
                self.invalidate_layout();
                self.project = if matches!(message, Message::Demo) {
                    Project::demo()
                } else {
                    Project::default()
                };
                self.part_filter.clear();
                self.selected = if self.project.parts.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.sync_stock();
                self.zoom.jump(1.);
                self.pan = Vector::ZERO;
                self.notice(
                    "Workspace ready. Import SVG or DXF parts. Undo restores the previous project.",
                    false,
                );
            }
            Message::Undo => {
                if let Some(project) = self.history.pop() {
                    self.invalidate_layout();
                    self.project = project;
                    self.dirty = true;
                    self.revision += 1;
                    self.selected = None;
                    self.sync_stock();
                    self.notice("Previous project state restored.", false);
                }
            }
            Message::SelectPart(index) if index < self.project.parts.len() => {
                self.selected = Some(index);
                self.library_tab = studio::LibraryTab::Parts;
                if !self.project.parts[index]
                    .name
                    .to_lowercase()
                    .contains(&self.part_filter.to_lowercase())
                {
                    self.part_filter.clear();
                }
            }
            Message::LibraryTab(tab) => self.library_tab = tab,
            Message::InspectorTab(tab) => self.inspector_tab = tab,
            Message::FilterParts(value) => self.part_filter = value,
            Message::Continuous(value) if self.worker.is_none() => {
                self.checkpoint();
                self.project.settings.continuous = value;
            }
            Message::Placement(value) if self.worker.is_none() => {
                self.checkpoint();
                self.project.settings.placement = value;
                self.invalidate_layout();
            }
            Message::Quantity(index, delta) => {
                if let Some(part) = self.project.parts.get(index) {
                    let quantity = (part.quantity as i32 + delta).clamp(0, 200) as u32;
                    if self.project.quantity() - part.quantity as usize + quantity as usize > 2000 {
                        self.notice("Maximum 2,000 copies per project.", true);
                    } else if quantity != part.quantity {
                        self.checkpoint();
                        self.invalidate_layout();
                        self.project.parts[index].quantity = quantity;
                    }
                }
            }
            Message::RemovePart(index) if index < self.project.parts.len() => {
                self.checkpoint();
                self.invalidate_layout();
                self.project.parts.remove(index);
                self.selected = None;
                self.notice("Part removed. Use Undo to restore it.", false);
            }
            Message::StockInput(field, value) => {
                self.stock_inputs[field as usize] = value;
                self.dirty = true;
                self.revision += 1;
            }
            Message::CommitStock => {
                self.commit_stock();
            }
            Message::Rotations(value) if (1..=32).contains(&value) => {
                self.checkpoint();
                self.invalidate_layout();
                self.project.rotations = value;
            }
            Message::Attempts(value) if (1..=100_000).contains(&value) => {
                self.checkpoint();
                self.run.stop();
                self.worker = None;
                self.project.attempts = value;
                self.project.settings.continuous = false;
                self.notice(
                    "Search budget updated. Run Nest to refine the layout.",
                    false,
                );
            }
            Message::Nest if self.worker.is_none() && !self.busy_file => {
                if !self.commit_stock() {
                    return Task::none();
                }
                if self.project.quantity() == 0 {
                    self.notice(
                        "Import parts or increase their quantities before nesting.",
                        true,
                    );
                } else {
                    self.progress = 0.0;
                    self.pass = 1;
                    self.run.begin(self.layout.as_ref(), &self.project);
                    self.worker = Some(nesting::Worker::start(self.project.clone()));
                    self.notice("Finding an efficient arrangement…", false);
                }
            }
            Message::Stop => {
                if self.settings_dialog.take().is_some() {
                    return Task::none();
                }
                self.show_help = false;
                self.show_project_menu = false;
                self.menu_progress.retarget(0., now, motion::panel());
                if self.worker.take().is_some() {
                    self.run.stop();
                    self.notice("Nesting stopped. The best completed layout is kept.", false);
                }
            }
            Message::SelectSheet(sheet)
                if self.layout.as_ref().is_some_and(|l| sheet < l.sheets.len()) =>
            {
                self.sheet = sheet
            }
            Message::Zoom(factor) if factor.is_finite() && factor > 0.0 => {
                self.zoom_anchor = None;
                self.zoom.retarget(
                    (self.zoom.target * factor).clamp(0.25, 5.0),
                    now,
                    motion::camera(),
                );
            }
            Message::ZoomAt(factor, cursor, viewport) if factor.is_finite() && factor > 0.0 => {
                let anchor = camera::Anchor::new(
                    &self.project.stock,
                    viewport,
                    self.zoom.current,
                    self.pan,
                    cursor,
                );
                self.zoom.retarget(
                    (self.zoom.target * factor).clamp(0.25, 5.0),
                    now,
                    motion::camera(),
                );
                self.pan = anchor.pan(&self.project.stock, self.zoom.current);
                self.zoom_anchor = self.zoom.is_active().then_some(anchor);
            }
            Message::Fit => {
                self.zoom_anchor = None;
                self.zoom.retarget(1.0, self.now(), motion::camera());
                self.pan = Vector::ZERO;
            }
            Message::Pan(pan) => self.pan = pan,
            Message::StartPan => self.stop_zoom(),
            Message::ToggleProjectMenu => {
                self.show_project_menu = !self.show_project_menu;
                self.menu_progress.retarget(
                    if self.show_project_menu { 1. } else { 0. },
                    now,
                    motion::panel(),
                );
            }
            Message::HintEnter(label, bounds) => {
                self.tooltip.enter(label, bounds, self.window_size, now)
            }
            Message::HintLeave(label) => self.tooltip.leave(label, now),
            Message::Resized(size) => {
                self.stop_zoom();
                self.window_size = size;
                return window::latest().and_then(refresh_window);
            }
            Message::Unfocused => self.stop_zoom(),
            Message::ToggleHelp => self.show_help = !self.show_help,
            Message::ToggleGrid => self.show_grid = !self.show_grid,
            Message::FocusNext(backwards) => {
                return if backwards {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                };
            }
            _ => (),
        }
        // Keep the ring alive through completion and reset, even after the
        // worker's polling subscription has ended. Unchanged targets must not
        // restart the spring on each frame.
        let target = if self.worker.is_some() {
            self.progress.clamp(0., 1.)
        } else if self.layout.is_some() {
            1.
        } else {
            0.72
        };
        if self.nest_progress.target != target {
            self.nest_progress.retarget(target, now, motion::progress());
        }
        Task::none()
    }
}

fn refresh_window(id: window::Id) -> Task<Message> {
    Task::batch([
        window::run(id, windowing::install_native_titlebar).map(Message::NativeTitlebarInstalled),
        window::is_maximized(id).map(Message::WindowMaximized),
    ])
}

async fn import_dialog(settings: settings::Settings) -> Result<Option<ImportBatch>, String> {
    let Some(files) = rfd::AsyncFileDialog::new()
        .add_filter("Vector outlines", &["svg", "dxf"])
        .add_filter("SVG", &["svg"])
        .add_filter("DXF", &["dxf"])
        .pick_files()
        .await
    else {
        return Ok(None);
    };
    read_vectors(
        files.into_iter().map(|f| f.path().to_owned()).collect(),
        settings,
    )
    .await
}

async fn read_vectors(
    paths: Vec<PathBuf>,
    settings: settings::Settings,
) -> Result<Option<ImportBatch>, String> {
    tokio::task::spawn_blocking(move || {
        let mut batch = ImportBatch {
            parts: vec![],
            skipped: 0,
            files: paths.len(),
            assumed_mm: 0,
        };
        for path in paths {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("part");
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let imported = match extension.as_str() {
                "svg" => svg_io::parse_with(
                    std::str::from_utf8(&bytes).map_err(|e| format!("Invalid SVG text: {e}"))?,
                    name,
                    &settings,
                ),
                "dxf" => dxf_io::parse_with(&bytes, name, &settings),
                _ => return Err("Import a local SVG or DXF file.".into()),
            }
            .map_err(|e| format!("{name}: {e}"))?;
            batch.assumed_mm += usize::from(imported.assumed_mm);
            batch.parts.extend(imported.parts);
            batch.skipped += imported.skipped;
        }
        Ok(Some(batch))
    })
    .await
    .map_err(|e| format!("Import failed: {e}"))?
}

async fn read_project(path: PathBuf) -> Result<Project, String> {
    tokio::task::spawn_blocking(move || {
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("Cannot open project: {e}"))?;
        let project: Project =
            serde_json::from_str(&source).map_err(|e| format!("Invalid project file: {e}"))?;
        project.validate()?;
        Ok(project)
    })
    .await
    .map_err(|e| format!("Cannot open project: {e}"))?
}

async fn save_dialog(
    name: &'static str,
    filter: &'static str,
    extension: &'static str,
    contents: String,
) -> Result<Option<String>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name(name)
        .add_filter(filter, &[extension])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    file.write(contents.as_bytes())
        .await
        .map_err(|e| format!("Cannot save file: {e}"))?;
    Ok(Some(file.path().display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> Layout {
        Layout {
            sheets: vec![vec![model::PlacedPart {
                part: 0,
                instance: 0,
                x: 10.,
                y: 10.,
                rotation: 0.,
            }]],
            fitness: 1.,
            elapsed: 0.1,
            pass: 1,
        }
    }

    #[test]
    fn quantity_edits_discard_stale_layouts_and_undo_restores_geometry() {
        let (mut app, _) = App::new();
        app.layout = Some(sample_layout());
        let _ = app.update(Message::Quantity(0, 1));
        assert_eq!(app.project.parts[0].quantity, 5);
        assert!(app.layout.is_none());
        let _ = app.update(Message::Undo);
        assert_eq!(app.project.parts[0].quantity, 4);
        assert!(app.layout.is_none());
    }

    #[test]
    fn invalid_stock_keeps_the_last_valid_geometry_until_corrected() {
        let (mut app, _) = App::new();
        app.layout = Some(sample_layout());
        let _ = app.update(Message::StockInput(StockField::Width, "bad".into()));
        assert!(!app.commit_stock());
        assert_eq!(app.project.stock.width, 480.);
        assert!(app.layout.is_some());
        assert!(app.stock_pending());
        let _ = app.update(Message::StockInput(StockField::Width, "600".into()));
        assert!(app.commit_stock());
        assert_eq!(app.project.stock.width, 600.);
        assert!(app.layout.is_none());
        assert!(!app.stock_pending());
    }

    #[test]
    fn saving_an_older_revision_does_not_mark_new_edits_saved() {
        let (mut app, _) = App::new();
        let saved_revision = app.revision;
        let _ = app.update(Message::Quantity(0, 1));
        let _ = app.update(Message::Saved(
            saved_revision,
            Ok(Some("sample.deepnest".into())),
        ));
        assert!(app.dirty);
        let _ = app.update(Message::Saved(
            app.revision,
            Ok(Some("sample.deepnest".into())),
        ));
        assert!(!app.dirty);
    }

    #[test]
    fn workspace_view_preferences_do_not_change_the_cutting_project() {
        let (mut app, _) = App::new();
        let before = serde_json::to_string(&app.project).unwrap();
        for message in [
            Message::ToggleGrid,
            Message::ToggleHelp,
            Message::LibraryTab(studio::LibraryTab::Stock),
            Message::InspectorTab(studio::InspectorTab::Insights),
        ] {
            let _ = app.update(message);
        }
        assert_eq!(serde_json::to_string(&app.project).unwrap(), before);
        assert!(!app.dirty);
    }

    #[test]
    fn animated_wheel_zoom_holds_the_mouse_point_and_dragging_takes_over() {
        let (mut app, _) = App::new();
        let viewport = Size::new(1100., 760.);
        let cursor = Point::new(210., 570.);
        app.pan = Vector::new(87., -42.);
        app.zoom.jump(1.3);
        let (scale, origin) =
            camera::transform(&app.project.stock, viewport, app.zoom.current, app.pan);
        let world = (cursor - origin) * (1. / scale);
        let _ = app.update(Message::ZoomAt(1.8, cursor, viewport));
        for millis in [0, 40, 80, 160, 300, 1000] {
            let _ = app.update(Message::Frame(app.epoch + Duration::from_millis(millis)));
            let (scale, origin) =
                camera::transform(&app.project.stock, viewport, app.zoom.current, app.pan);
            let actual = origin + world * scale;
            assert!((actual.x - cursor.x).abs() < 0.001);
            assert!((actual.y - cursor.y).abs() < 0.001);
        }
        assert!(app.zoom_anchor.is_none());
        let _ = app.update(Message::ZoomAt(0.5, cursor, viewport));
        let (zoom, pan) = (app.zoom.current, app.pan);
        let _ = app.update(Message::StartPan);
        let _ = app.update(Message::Frame(app.epoch + Duration::from_secs(2)));
        assert!(!app.zoom.is_active());
        assert!(app.zoom_anchor.is_none());
        assert_eq!(app.zoom.current, zoom);
        assert_eq!(app.pan, pan);
        let _ = app.update(Message::Pan(pan + Vector::new(25., -10.)));
        assert_eq!(app.pan, pan + Vector::new(25., -10.));
        assert!(!app.dirty);
    }

    #[test]
    fn progress_ring_animates_completion_and_reset_after_worker_stops() {
        let (mut app, _) = App::new();
        app.nest_progress.jump(0.35);
        app.layout = Some(sample_layout());
        app.progress = 1.;
        let started = Instant::now();
        let _ = app.update(Message::Poll);
        assert!(app.worker.is_none());
        assert_eq!(app.nest_progress.current, 0.35);
        assert_eq!(app.nest_progress.target, 1.);
        assert!(app.nest_progress.is_active());

        let mut previous = 0.35;
        for millis in [16, 48, 96, 160, 300, 1000] {
            let _ = app.update(Message::Frame(started + Duration::from_millis(millis)));
            assert!(app.nest_progress.current >= previous);
            assert!(app.nest_progress.current <= 1.);
            previous = app.nest_progress.current;
        }
        assert_eq!(app.nest_progress.current, 1.);
        assert!(!app.nest_progress.is_active());
        let _ = app.update(Message::Poll);
        assert!(!app.nest_progress.is_active());

        let reset = Instant::now();
        let _ = app.update(Message::Quantity(0, 1));
        assert!(app.layout.is_none());
        assert_eq!(app.nest_progress.target, 0.72);
        assert_eq!(app.nest_progress.current, 1.);
        let _ = app.update(Message::Frame(reset + Duration::from_millis(40)));
        assert!(app.nest_progress.current > 0.72 && app.nest_progress.current < 1.);
        let _ = app.update(Message::Frame(reset + Duration::from_secs(1)));
        assert_eq!(app.nest_progress.current, 0.72);
        assert!(!app.nest_progress.is_active());
    }

    #[test]
    fn refinement_keeps_the_incumbent_until_a_better_valid_result_arrives() {
        let (mut app, _) = App::new();
        let mut initial = sample_layout();
        initial.fitness = 10.;
        app.accept_layout(initial.clone());
        let mut worse = initial.clone();
        worse.fitness = 20.;
        app.accept_layout(worse);
        assert_eq!(app.layout.as_ref().unwrap().fitness, 10.);
        assert_eq!(app.run.history.len(), 1);
        let mut better = initial;
        better.fitness = 5.;
        app.accept_layout(better);
        assert_eq!(app.layout.as_ref().unwrap().fitness, 5.);
        assert_eq!(app.run.history.len(), 2);
    }

    #[test]
    fn settings_apply_cancel_validate_and_undo_without_rescaling_parts() {
        use settings::{Action, Number, Units};
        let (mut app, _) = App::new();
        app.layout = Some(sample_layout());
        let before = serde_json::to_string(&app.project).unwrap();
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::Input(
            Number::Rotations,
            "7".into(),
        )));
        let _ = app.update(Message::Settings(Action::Close));
        assert_eq!(serde_json::to_string(&app.project).unwrap(), before);
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::Units(Units::Inches)));
        let _ = app.update(Message::Settings(Action::Apply));
        assert_eq!(app.project.settings.units, Units::Inches);
        assert!(app.layout.is_some());
        assert!(!app.stock_pending());
        assert_eq!(app.project.stock.width, 480.);
        assert_eq!(app.project.stock.gap, 3.);
        assert_eq!(app.project.settings.curve_tolerance, 0.1);
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::Input(
            Number::Population,
            "2.5".into(),
        )));
        let _ = app.update(Message::Settings(Action::Apply));
        assert!(app.settings_dialog.as_ref().unwrap().error.is_some());
        assert_eq!(app.project.settings.population, 10);
        let _ = app.update(Message::Settings(Action::Input(
            Number::Population,
            "24".into(),
        )));
        let _ = app.update(Message::Settings(Action::Input(
            Number::Rotations,
            "7".into(),
        )));
        let _ = app.update(Message::Settings(Action::Apply));
        assert_eq!(app.project.rotations, 7);
        assert_eq!(app.project.settings.population, 24);
        assert!(app.layout.is_none());
        let saved = serde_json::to_string(&app.project).unwrap();
        let restored: Project = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.settings, app.project.settings);
        let _ = app.update(Message::Undo);
        assert_eq!(app.project.settings.population, 10);
        assert_eq!(app.project.rotations, 4);
    }

    #[test]
    fn inch_inputs_change_physical_stock_and_reset_is_only_a_draft() {
        use settings::{Action, Units};
        let (mut app, _) = App::new();
        app.project.settings.units = Units::Inches;
        app.sync_stock();
        let _ = app.update(Message::StockInput(StockField::Width, "20".into()));
        assert!(app.commit_stock());
        assert_eq!(app.project.stock.width, 508.);
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::Reset));
        assert_eq!(app.project.settings.units, Units::Inches);
        let _ = app.update(Message::Settings(Action::Apply));
        assert_eq!(app.project.settings.units, Units::Millimeters);
        assert_eq!(app.project.stock.width, 508.);
    }
}
