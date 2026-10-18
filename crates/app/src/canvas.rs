use crate::{
    model::{transformed, Part, PlacedPart, Project},
    style::alpha,
    tokens::{color::*, typography::*},
    App, Message,
};
use iced::{
    alignment, mouse,
    widget::canvas::{self, path::Arc, Frame, Geometry, LineCap, Path, Stroke},
    Color, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use nest::Pt;

pub fn preview_placements(project: &Project) -> Vec<PlacedPart> {
    let (mut x, mut y, mut row_height) = (20.0, 20.0, 0.0_f64);
    project
        .parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.quantity > 0)
        .map(|(i, part)| {
            let b = part.bounds();
            if x > 20.0 && x + b.width > project.stock.width - 20.0 {
                x = 20.0;
                y += row_height + 24.0;
                row_height = 0.0;
            }
            let placement = PlacedPart {
                part: i,
                instance: i as i64 * 1000,
                x: x - b.x,
                y: y - b.y,
                rotation: 0.0,
            };
            x += b.width + 24.0;
            row_height = row_height.max(b.height);
            placement
        })
        .collect()
}

fn path(part: &Part, map: impl Fn(Pt) -> Point) -> Path {
    Path::new(|builder| {
        for ring in std::iter::once(&part.points).chain(&part.holes) {
            if let Some(first) = ring.first() {
                builder.move_to(map(*first));
                for point in ring.iter().skip(1) {
                    builder.line_to(map(*point));
                }
                builder.close();
            }
        }
    })
}

fn fill(color: Color) -> canvas::Fill {
    canvas::Fill {
        style: canvas::Style::Solid(color),
        rule: canvas::fill::Rule::EvenOdd,
    }
}

fn label(frame: &mut Frame, content: impl Into<String>, position: Point, size: f32, color: Color) {
    frame.fill_text(canvas::Text {
        content: content.into(),
        position,
        size: size.into(),
        color,
        font: MONO,
        ..Default::default()
    });
}

pub struct Workspace<'a>(pub &'a App);

#[derive(Default)]
pub struct CanvasState {
    drag: Option<(Point, Vector)>,
}

impl Workspace<'_> {
    fn transform(&self, bounds: Size) -> (f32, Point) {
        crate::camera::transform(
            &self.0.project.stock,
            bounds,
            self.0.zoom.current,
            self.0.pan,
        )
    }

    fn placements(&self) -> Vec<PlacedPart> {
        if let Some(layout) = &self.0.layout {
            layout
                .sheets
                .get(self.0.sheet)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|mut p| {
                    if let Some(pose) = self.0.poses.get(&p.instance) {
                        p.x = pose.x.current;
                        p.y = pose.y.current;
                        p.rotation = pose.rotation.current;
                    }
                    p
                })
                .collect()
        } else {
            preview_placements(&self.0.project)
        }
    }

    fn hit(&self, cursor: Point, bounds: Size) -> Option<usize> {
        let (scale, origin) = self.transform(bounds);
        let world = Pt::new(
            ((cursor.x - origin.x) / scale) as f64,
            ((cursor.y - origin.y) / scale) as f64,
        );
        self.placements().iter().rev().find_map(|placement| {
            let (sin, cos) = (-placement.rotation).to_radians().sin_cos();
            let (x, y) = (world.x - placement.x, world.y - placement.y);
            let point = Pt::new(x * cos - y * sin, x * sin + y * cos);
            let part = &self.0.project.parts[placement.part];
            (crate::contours::contains(&part.points, point)
                && !part
                    .holes
                    .iter()
                    .any(|hole| crate::contours::contains(hole, point)))
            .then_some(placement.part)
        })
    }
}

impl canvas::Program<Message> for Workspace<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonReleased(_)) if state.drag.is_some() => {
                state.drag = None;
                return Some(canvas::Action::capture());
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position })
                if state.drag.is_some() =>
            {
                let (start, pan) = state.drag.unwrap();
                return Some(
                    canvas::Action::publish(Message::Pan(pan + (*position - start))).and_capture(),
                );
            }
            _ => (),
        }
        let position = cursor.position_in(bounds)?;
        match event {
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y * 0.12,
                    mouse::ScrollDelta::Pixels { y, .. } => *y * 0.004,
                };
                Some(
                    canvas::Action::publish(Message::ZoomAt(
                        (amount as f64).exp(),
                        position,
                        bounds.size(),
                    ))
                    .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(button))
                if *button == mouse::Button::Left || *button == mouse::Button::Middle =>
            {
                if *button == mouse::Button::Left {
                    if let Some(part) = self.hit(position, bounds.size()) {
                        return Some(
                            canvas::Action::publish(Message::SelectPart(part)).and_capture(),
                        );
                    }
                }
                state.drag = cursor.position().map(|p| (p, self.0.pan));
                Some(canvas::Action::publish(Message::StartPan).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _: &Self::State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let mut labels = Frame::new(renderer, bounds.size());
        let size = frame.size();
        frame.fill_rectangle(Point::ORIGIN, size, CANVAS);
        if self.0.show_grid {
            for x in (12..size.width as usize).step_by(24) {
                for y in (12..size.height as usize).step_by(24) {
                    frame.fill(&Path::circle(Point::new(x as f32, y as f32), 0.7), GRID);
                }
            }
        }
        let (scale, origin) = self.transform(size);
        let stock = &self.0.project.stock;
        let sheet_size = Size::new(stock.width as f32 * scale, stock.height as f32 * scale);
        // Keep geometry in one frame: nested clipped frames can reorder meshes
        // behind the parent's background in Iced's WGPU renderer.
        frame.fill_rectangle(origin, sheet_size, SHEET);
        frame.stroke(
            &Path::rectangle(origin, sheet_size),
            Stroke::default().with_width(1.).with_color(SHEET_BORDER),
        );
        // Quiet points inside the stock retain its scale without competing with cuts.
        if self.0.show_grid {
            let spacing = (20.0 * scale).max(12.0);
            let mut x = origin.x + spacing;
            while x < origin.x + sheet_size.width {
                let mut y = origin.y + spacing;
                while y < origin.y + sheet_size.height {
                    frame.fill(&Path::circle(Point::new(x, y), 0.65), alpha(CUT_LINE, 0.16));
                    y += spacing;
                }
                x += spacing;
            }
        }
        // Registration corners give the material a precise, illuminated edge.
        for (x, y, dx, dy) in [
            (origin.x, origin.y, 1., 1.),
            (origin.x + sheet_size.width, origin.y, -1., 1.),
            (origin.x, origin.y + sheet_size.height, 1., -1.),
            (
                origin.x + sheet_size.width,
                origin.y + sheet_size.height,
                -1.,
                -1.,
            ),
        ] {
            frame.stroke(
                &Path::new(|p| {
                    p.move_to(Point::new(x, y + 12. * dy));
                    p.line_to(Point::new(x, y));
                    p.line_to(Point::new(x + 12. * dx, y));
                }),
                Stroke::default()
                    .with_width(1.5)
                    .with_color(alpha(PRIMARY, 0.65)),
            );
        }

        // Choose ruler spacing in physical units, keeping labels ~80px apart.
        let units = self.0.project.settings.units;
        let raw_step = (80.0 / scale as f64 / units.factor()).max(0.001);
        let magnitude = 10_f64.powf(raw_step.log10().floor());
        let step = [1., 2., 5., 10.]
            .into_iter()
            .map(|v| v * magnitude)
            .find(|v| *v >= raw_step)
            .unwrap()
            * units.factor();
        let mut mm = 0.0;
        while mm <= stock.width {
            let x = origin.x + mm as f32 * scale;
            frame.stroke(
                &Path::line(Point::new(x, origin.y - 7.), Point::new(x, origin.y - 2.)),
                Stroke::default().with_color(SUBTLE),
            );
            if x > 48. && x < size.width - 24. {
                label(
                    &mut labels,
                    units.display(mm),
                    Point::new(x, origin.y - 25.),
                    CAPTION,
                    MUTED_FOREGROUND,
                );
            }
            mm += step;
        }
        mm = 0.0;
        while mm <= stock.height {
            let y = origin.y + mm as f32 * scale;
            frame.stroke(
                &Path::line(Point::new(origin.x - 7., y), Point::new(origin.x - 2., y)),
                Stroke::default().with_color(SUBTLE),
            );
            if y > 72. && y < size.height - 64. {
                labels.fill_text(canvas::Text {
                    content: units.display(mm),
                    position: Point::new(origin.x - 14., y),
                    align_x: iced::widget::text::Alignment::Right,
                    align_y: alignment::Vertical::Center,
                    color: MUTED_FOREGROUND,
                    size: CAPTION.into(),
                    font: MONO,
                    ..Default::default()
                });
            }
            mm += step;
        }
        for placement in self.placements() {
            let part = &self.0.project.parts[placement.part];
            let shape = path(part, |p| {
                let p = transformed(p, &placement);
                origin + Vector::new(p.x as f32 * scale, p.y as f32 * scale)
            });
            let selected = self.0.selected == Some(placement.part);
            frame.fill(
                &shape,
                fill(if selected {
                    SELECTED_PART
                } else {
                    alpha(PARTS[placement.part % PARTS.len()], 0.16)
                }),
            );
            frame.stroke(
                &shape,
                Stroke::default()
                    .with_width(if selected { 1.8 } else { 1.0 })
                    .with_color(if selected {
                        SELECTED_CUT_LINE
                    } else {
                        alpha(PARTS[placement.part % PARTS.len()], 0.85)
                    }),
            );
            let b = part.bounds();
            if scale * b.width as f32 > 48.0 && scale * b.height as f32 > 32.0 {
                // Keep identifiers on material, including concave parts/rings.
                let center = [(0.5, 0.5), (0.25, 0.25), (0.2, 0.7), (0.5, 0.15)]
                    .into_iter()
                    .map(|(x, y)| Pt::new(b.x + b.width * x, b.y + b.height * y))
                    .find(|&p| {
                        crate::contours::contains(&part.points, p)
                            && !part.holes.iter().any(|h| crate::contours::contains(h, p))
                    });
                if let Some(center) = center {
                    let center = transformed(center, &placement);
                    labels.fill_text(canvas::Text {
                        content: format!("{:02}", placement.part + 1),
                        position: origin
                            + Vector::new(center.x as f32 * scale, center.y as f32 * scale),
                        align_x: iced::widget::text::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        color: alpha(SHEET_FOREGROUND, 0.8),
                        size: CAPTION.into(),
                        font: MONO,
                        ..Default::default()
                    });
                }
            }
        }
        // Text gets a final pass so rulers remain visible outside the paper.
        vec![frame.into_geometry(), labels.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() {
            mouse::Interaction::Grabbing
        } else if let Some(position) = cursor.position_in(bounds) {
            if self.hit(position, bounds.size()).is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Grab
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

pub struct Thumbnail<'a> {
    pub part: &'a Part,
    pub index: usize,
}
impl canvas::Program<Message> for Thumbnail<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let b = self.part.bounds();
        let scale = ((bounds.width - 8.) / b.width.max(1.) as f32)
            .min((bounds.height - 8.) / b.height.max(1.) as f32);
        let origin = Point::new(
            (bounds.width - b.width as f32 * scale) / 2.,
            (bounds.height - b.height as f32 * scale) / 2.,
        );
        let path = path(self.part, |p| {
            origin + Vector::new((p.x - b.x) as f32 * scale, (p.y - b.y) as f32 * scale)
        });
        frame.fill(&path, fill(alpha(PARTS[self.index % PARTS.len()], 0.18)));
        frame.stroke(
            &path,
            Stroke::default()
                .with_width(1.)
                .with_color(PARTS[self.index % PARTS.len()]),
        );
        vec![frame.into_geometry()]
    }
}

/// A quiet halo around the real nesting action, sampled by the shared motion loop.
pub struct NestRing {
    pub progress: f64,
}
impl canvas::Program<Message> for NestRing {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2., bounds.height / 2.);
        let diameter = bounds.width.min(bounds.height);
        let ring_radius = diameter * 0.4125;
        for (ratio, opacity) in [(0.48125, 0.025), (0.4125, 0.035), (0.35, 0.055)] {
            frame.fill(
                &Path::circle(center, diameter * ratio),
                alpha(PRIMARY, opacity),
            );
        }
        frame.stroke(
            &Path::circle(center, ring_radius),
            Stroke::default()
                .with_width(3.)
                .with_color(alpha(PRIMARY, 0.12)),
        );
        let sweep = self.progress.clamp(0., 1.) as f32;
        let start = -std::f32::consts::FRAC_PI_2;
        let end = start + std::f32::consts::TAU * sweep;
        let ring = Path::new(|p| {
            p.arc(Arc {
                center,
                radius: ring_radius,
                start_angle: iced::Radians(start),
                end_angle: iced::Radians(end),
            });
        });
        frame.stroke(
            &ring,
            Stroke::default()
                .with_width(3.)
                .with_line_cap(LineCap::Round)
                .with_color(alpha(PRIMARY, 0.7)),
        );
        let tip = center + Vector::new(end.cos() * ring_radius, end.sin() * ring_radius);
        frame.fill(&Path::circle(tip, 3.), PRIMARY);
        vec![frame.into_geometry()]
    }
}

/// Small sheet previews use the original cut contours, including interior holes.
pub struct SheetPreview<'a> {
    pub project: &'a Project,
    pub placements: &'a [PlacedPart],
}
impl canvas::Program<Message> for SheetPreview<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let stock = &self.project.stock;
        let scale = ((bounds.width - 8.) / stock.width as f32)
            .min((bounds.height - 8.) / stock.height as f32);
        let size = Size::new(stock.width as f32 * scale, stock.height as f32 * scale);
        let origin = Point::new(
            (bounds.width - size.width) / 2.,
            (bounds.height - size.height) / 2.,
        );
        frame.fill_rectangle(origin, size, SHEET);
        frame.stroke(
            &Path::rectangle(origin, size),
            Stroke::default().with_color(SHEET_BORDER).with_width(1.),
        );
        for placed in self.placements {
            let outline = path(&self.project.parts[placed.part], |point| {
                let point = transformed(point, placed);
                origin + Vector::new(point.x as f32 * scale, point.y as f32 * scale)
            });
            let color = PARTS[placed.part % PARTS.len()];
            frame.fill(&outline, fill(alpha(color, 0.32)));
            frame.stroke(
                &outline,
                Stroke::default()
                    .with_color(alpha(color, 0.8))
                    .with_width(0.65),
            );
        }
        vec![frame.into_geometry()]
    }
}

/// Observed utilization over elapsed search time; no synthetic history points.
pub struct SearchTrend<'a> {
    pub samples: &'a [crate::studio::Sample],
    pub elapsed: f64,
}
impl canvas::Program<Message> for SearchTrend<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = ((bounds.width - 8.).max(1.), (bounds.height - 10.).max(1.));
        for i in 0..=3 {
            let y = 5. + h * i as f32 / 3.;
            frame.stroke(
                &Path::line(Point::new(4., y), Point::new(w + 4., y)),
                Stroke::default()
                    .with_width(1.)
                    .with_color(alpha(BORDER, 0.7)),
            );
        }
        if let Some(first) = self.samples.first() {
            let duration = self
                .elapsed
                .max(self.samples.last().unwrap().elapsed)
                .max(0.01);
            let point = |sample: &crate::studio::Sample| {
                Point::new(
                    4. + (sample.elapsed / duration) as f32 * w,
                    5. + h * (1. - sample.utilization.clamp(0., 100.) as f32 / 100.),
                )
            };
            let first_point = point(first);
            let last_point = point(self.samples.last().unwrap());
            let line = Path::new(|p| {
                p.move_to(first_point);
                let mut previous = first_point;
                for sample in self.samples.iter().skip(1) {
                    let next = point(sample);
                    p.line_to(Point::new(next.x, previous.y));
                    p.line_to(next);
                    previous = next;
                }
                p.line_to(Point::new(w + 4., last_point.y));
            });
            frame.stroke(
                &line,
                Stroke::default()
                    .with_width(2.)
                    .with_line_cap(LineCap::Round)
                    .with_color(PRIMARY),
            );
            frame.fill(&Path::circle(Point::new(w + 4., last_point.y), 3.), PRIMARY);
        }
        vec![frame.into_geometry()]
    }
}
