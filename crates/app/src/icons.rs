//! A consistent line icon set drawn by Iced, without an icon-font dependency.
use crate::{
    tokens::{color::*, layout},
    Message,
};
use iced::{
    mouse,
    widget::{
        canvas,
        canvas::{Frame, Geometry, Path, Stroke},
    },
    Color, Point, Rectangle, Renderer, Size, Theme,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Logo,
    Plus,
    Minus,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    Check,
    Search,
    Chart,
    Layers,
    Import,
    Export,
    Save,
    Undo,
    Play,
    Stop,
    Fit,
    Grid,
    Window,
    Restore,
    Trash,
    Help,
    Close,
    File,
    Folder,
    Settings,
}

pub fn icon(kind: Icon, color: Color) -> canvas::Canvas<Glyph, Message> {
    canvas(Glyph { kind, color })
        .width(layout::ICON)
        .height(layout::ICON)
}

pub struct Glyph {
    kind: Icon,
    color: Color,
}
impl canvas::Program<Message> for Glyph {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        f.scale(bounds.width / 20.0);
        let stroke = Stroke::default().with_width(1.5).with_color(self.color);
        let line = |f: &mut Frame, points: &[(f32, f32)]| {
            f.stroke(
                &Path::new(|p| {
                    p.move_to(Point::from(points[0]));
                    for point in &points[1..] {
                        p.line_to(Point::from(*point));
                    }
                }),
                stroke,
            );
        };
        let rect = |f: &mut Frame, x, y, w, h| {
            f.stroke(
                &Path::rounded_rectangle(Point::new(x, y), Size::new(w, h), 2.0.into()),
                stroke,
            )
        };
        match self.kind {
            Icon::Settings => {
                for (y, x) in [(5., 7.), (10., 13.), (15., 8.)] {
                    line(&mut f, &[(2., y), (18., y)]);
                    f.fill(&Path::circle(Point::new(x, y), 2.5), SIDEBAR);
                    f.stroke(&Path::circle(Point::new(x, y), 2.5), stroke);
                }
            }
            Icon::Logo => {
                f.fill(
                    &Path::rounded_rectangle(Point::new(1., 1.), Size::new(10., 11.), 2.0.into()),
                    PRIMARY,
                );
                f.fill(
                    &Path::rounded_rectangle(Point::new(13., 1.), Size::new(6., 6.), 1.5.into()),
                    self.color.scale_alpha(0.65),
                );
                f.fill(
                    &Path::rounded_rectangle(Point::new(1., 14.), Size::new(10., 5.), 1.5.into()),
                    self.color.scale_alpha(0.65),
                );
                f.fill(
                    &Path::rounded_rectangle(Point::new(13., 9.), Size::new(6., 10.), 1.5.into()),
                    PRIMARY,
                );
            }
            Icon::Plus => {
                line(&mut f, &[(4., 10.), (16., 10.)]);
                line(&mut f, &[(10., 4.), (10., 16.)]);
            }
            Icon::Minus => line(&mut f, &[(4., 10.), (16., 10.)]),
            Icon::ChevronDown => line(&mut f, &[(6., 8.), (10., 12.), (14., 8.)]),
            Icon::ChevronRight => line(&mut f, &[(8., 6.), (12., 10.), (8., 14.)]),
            Icon::ChevronLeft => line(&mut f, &[(12., 6.), (8., 10.), (12., 14.)]),
            Icon::Check => line(&mut f, &[(4., 10.), (8., 14.), (16., 6.)]),
            Icon::Search => {
                f.stroke(&Path::circle(Point::new(8., 8.), 5.), stroke);
                line(&mut f, &[(12., 12.), (17., 17.)]);
            }
            Icon::Chart => {
                line(&mut f, &[(3., 3.), (3., 17.), (18., 17.)]);
                line(&mut f, &[(5., 13.), (9., 9.), (12., 11.), (17., 5.)]);
            }
            Icon::Layers => {
                line(
                    &mut f,
                    &[(2., 6.), (10., 2.), (18., 6.), (10., 10.), (2., 6.)],
                );
                line(&mut f, &[(2., 10.), (10., 14.), (18., 10.)]);
                line(&mut f, &[(2., 14.), (10., 18.), (18., 14.)]);
            }
            Icon::Import => {
                line(&mut f, &[(3., 12.), (3., 17.), (17., 17.), (17., 12.)]);
                line(&mut f, &[(10., 2.), (10., 12.)]);
                line(&mut f, &[(6., 8.), (10., 12.), (14., 8.)]);
            }
            Icon::Export => {
                line(&mut f, &[(3., 12.), (3., 17.), (17., 17.), (17., 12.)]);
                line(&mut f, &[(10., 12.), (10., 2.)]);
                line(&mut f, &[(6., 6.), (10., 2.), (14., 6.)]);
            }
            Icon::Save => {
                line(
                    &mut f,
                    &[
                        (3., 3.),
                        (14., 3.),
                        (17., 6.),
                        (17., 17.),
                        (3., 17.),
                        (3., 3.),
                    ],
                );
                rect(&mut f, 6., 3., 7., 5.);
                rect(&mut f, 6., 11., 8., 6.);
            }
            Icon::Undo => {
                line(&mut f, &[(7., 3.), (3., 7.), (7., 11.)]);
                f.stroke(
                    &Path::new(|p| {
                        p.move_to(Point::new(3., 7.));
                        p.line_to(Point::new(12., 7.));
                        p.bezier_curve_to(
                            Point::new(19., 7.),
                            Point::new(19., 17.),
                            Point::new(12., 17.),
                        );
                    }),
                    stroke,
                );
            }
            Icon::Play => f.fill(
                &Path::new(|p| {
                    p.move_to(Point::new(6., 3.));
                    p.line_to(Point::new(17., 10.));
                    p.line_to(Point::new(6., 17.));
                    p.close();
                }),
                self.color,
            ),
            Icon::Stop => f.fill(
                &Path::rounded_rectangle(Point::new(5., 5.), Size::new(10., 10.), 1.5.into()),
                self.color,
            ),
            Icon::Fit => {
                for points in [
                    &[(7., 3.), (3., 3.), (3., 7.)][..],
                    &[(13., 3.), (17., 3.), (17., 7.)],
                    &[(3., 13.), (3., 17.), (7., 17.)],
                    &[(13., 17.), (17., 17.), (17., 13.)],
                ] {
                    line(&mut f, points);
                }
            }
            Icon::Grid => {
                for x in [5., 10., 15.] {
                    for y in [5., 10., 15.] {
                        f.fill(&Path::circle(Point::new(x, y), 1.), self.color);
                    }
                }
            }
            Icon::Window => rect(&mut f, 3., 3., 14., 14.),
            Icon::Restore => {
                rect(&mut f, 3., 7., 10., 10.);
                line(
                    &mut f,
                    &[(7., 7.), (7., 3.), (17., 3.), (17., 13.), (13., 13.)],
                );
            }
            Icon::Trash => {
                line(&mut f, &[(3., 5.), (17., 5.)]);
                line(&mut f, &[(7., 5.), (7., 2.), (13., 2.), (13., 5.)]);
                line(&mut f, &[(5., 5.), (6., 17.), (14., 17.), (15., 5.)]);
            }
            Icon::Help => {
                f.stroke(&Path::circle(Point::new(10., 10.), 8.), stroke);
                line(
                    &mut f,
                    &[
                        (8., 7.),
                        (8., 6.),
                        (12., 6.),
                        (13., 8.),
                        (10., 10.),
                        (10., 11.),
                    ],
                );
                f.fill(&Path::circle(Point::new(10., 14.), 0.8), self.color);
            }
            Icon::Close => {
                line(&mut f, &[(5., 5.), (15., 15.)]);
                line(&mut f, &[(15., 5.), (5., 15.)]);
            }
            Icon::File => {
                line(
                    &mut f,
                    &[
                        (4., 2.),
                        (12., 2.),
                        (16., 6.),
                        (16., 18.),
                        (4., 18.),
                        (4., 2.),
                    ],
                );
                line(&mut f, &[(12., 2.), (12., 7.), (16., 7.)]);
            }
            Icon::Folder => line(
                &mut f,
                &[
                    (2., 5.),
                    (7., 5.),
                    (9., 7.),
                    (18., 7.),
                    (18., 16.),
                    (2., 16.),
                    (2., 5.),
                ],
            ),
        }
        vec![f.into_geometry()]
    }
}
