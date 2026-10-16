use crate::{
    model::{transformed, Part, PlacedPart, Project},
    style::alpha,
    tokens::{color::*, typography::*},
    App, Message,
};
use iced::{
    alignment, mouse,
    widget::canvas::{self, Frame, Geometry, Path, Stroke},
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
        frame.fill_rectangle(Point::ORIGIN, size, BACKGROUND);
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
        frame.fill_rectangle(origin + Vector::new(0., 8.), sheet_size, SHADOW);
        frame.fill_rectangle(origin, sheet_size, SHEET);
        frame.stroke(
            &Path::rectangle(origin, sheet_size),
            Stroke::default().with_width(1.).with_color(SHEET_BORDER),
        );

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
                    PARTS[placement.part % PARTS.len()]
                }),
            );
            frame.stroke(
                &shape,
                Stroke::default()
                    .with_width(if selected { 1.8 } else { 1.0 })
                    .with_color(if selected {
                        SELECTED_CUT_LINE
                    } else {
                        CUT_LINE
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
        label(
            &mut labels,
            if self.0.layout.is_some() {
                format!("SHEET {:02}", self.0.sheet + 1)
            } else {
                "SOURCE PREVIEW".into()
            },
            Point::new(origin.x, origin.y + sheet_size.height + 16.),
            CAPTION,
            SUBTLE,
        );
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
        frame.fill(&path, fill(alpha(PARTS[self.index % PARTS.len()], 0.75)));
        frame.stroke(
            &path,
            Stroke::default()
                .with_width(1.)
                .with_color(PARTS[self.index % PARTS.len()]),
        );
        vec![frame.into_geometry()]
    }
}
