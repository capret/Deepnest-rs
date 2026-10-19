//! A quiet settings surface with contextual, illustrated explanations.
use crate::{
    i18n::{trf, Language},
    icons::{icon, Icon},
    settings::{Action, CadUnit, Dialog, Help, Number, Placement, Section, Units},
    style::{self, Button},
    tokens::{color::*, radius, space as gap, typography::*},
    widgets, Message,
};
use iced::widget::canvas::{Frame, Geometry, Path, Stroke};
use iced::{
    alignment::Vertical,
    mouse,
    widget::{
        button, canvas, checkbox, column, container, mouse_area, opaque, pick_list, row, space,
        text, text_input, Column,
    },
    Element, Fill, Point, Rectangle, Renderer, Size, Theme,
};

fn message(action: Action) -> Message {
    Message::Settings(action)
}
fn label<'a>(l: Language, title: &'a str, hint: &'a str) -> Element<'a, Message> {
    column![
        text(l.tr(title)).size(BODY).font(BOLD),
        text(l.tr(hint)).size(SMALL).color(MUTED_FOREGROUND)
    ]
    .spacing(5)
    .width(Fill)
    .into()
}
fn setting<'a>(
    l: Language,
    title: &'a str,
    hint: &'a str,
    control: Element<'a, Message>,
    help: Help,
) -> Element<'a, Message> {
    mouse_area(
        container(
            row![
                label(l, title, hint),
                control,
                button(icon(Icon::Help, MUTED_FOREGROUND))
                    .padding(6)
                    .style(style::button_style(Button::Ghost))
                    .on_press(message(Action::Help(help))),
            ]
            .spacing(gap::SM)
            .align_y(Vertical::Center),
        )
        .padding([gap::SM, 0.]),
    )
    .on_enter(message(Action::Help(help)))
    .into()
}
fn numeric<'a>(
    l: Language,
    d: &'a Dialog,
    field: Number,
    title: &'a str,
    hint: &'a str,
    suffix: &'a str,
) -> Element<'a, Message> {
    let input = row![
        text_input("0", &d.inputs[field as usize])
            .on_input(move |s| message(Action::Input(field, s)))
            .on_submit(message(Action::Apply))
            .width(86)
            .size(BODY)
            .font(MONO)
            .padding(9)
            .style(style::input),
        text(l.tr(suffix)).size(SMALL).color(SUBTLE).width(34)
    ]
    .spacing(7)
    .align_y(Vertical::Center);
    setting(l, title, hint, input.into(), field.help())
}
fn toggle<'a>(
    l: Language,
    title: &'a str,
    hint: &'a str,
    value: bool,
    action: fn(bool) -> Action,
    help: Help,
) -> Element<'a, Message> {
    setting(
        l,
        title,
        hint,
        checkbox(value)
            .on_toggle(move |v| message(action(v)))
            .into(),
        help,
    )
}
fn line() -> Element<'static, Message> {
    container(space::horizontal().height(1))
        .style(|_| container::Style {
            background: Some(BORDER.into()),
            ..Default::default()
        })
        .into()
}

pub fn dialog(d: &Dialog, l: Language, window: Size) -> Element<'_, Message> {
    let mut tabs = row![].spacing(gap::XXS);
    for section in Section::ALL {
        tabs = tabs.push(
            button(text(l.tr(section.label())).size(BODY))
                .padding([9, 14])
                .on_press(message(Action::Section(section)))
                .style(style::button_style(if d.section == section {
                    Button::Selected
                } else {
                    Button::Ghost
                })),
        );
    }
    let unit = d.settings.units.symbol();
    let fields: Column<'_, Message> = match d.section {
        Section::Nesting => column![
            setting(l, "Display units", "Dimensions and rulers", pick_list(l.labels(&[Units::Millimeters, Units::Inches]), Some(l.label(d.settings.units)), |v| message(Action::Units(v.value)))
                .width(166).text_size(BODY).padding(9).style(style::select).into(), Help::Units),
            line(), numeric(l, d, Number::Gap, "Part gap", "Minimum clearance between cut outlines", unit),
            line(), numeric(l, d, Number::Rotations, "Allowed rotations", "1–32 evenly spaced orientations", "steps"),
            line(), setting(l, "Placement strategy", "Choose the shape of the remaining stock", pick_list(l.labels(&[Placement::Gravity, Placement::BoundingBox, Placement::Compact]), Some(l.label(d.settings.placement)), |v| message(Action::Placement(v.value)))
                .width(166).text_size(BODY).padding(9).style(style::select).into(), Help::Placement),
        ],
        Section::Geometry => column![
            numeric(l, d, Number::Curves, "Curve tolerance", "Maximum curve approximation error", unit), line(),
            numeric(l, d, Number::Endpoints, "Endpoint tolerance", "Join nearby open path endpoints", unit), line(),
            toggle(l, "Use rough approximation", "Nest a simpler, enclosing shape", d.settings.simplify, Action::Simplify, Help::Simplify),
            container(text(l.tr("Curve and endpoint tolerances apply on import. Reimport a file to rebuild its outlines with new tolerances.")).size(SMALL).color(MUTED_FOREGROUND))
                .padding(gap::SM).style(style::inset),
        ],
        Section::Files => column![
            numeric(l, d, Number::Svg, "SVG fallback scale", "For files without physical dimensions", "u/in"), line(),
            setting(l, "DXF import units", "Read declared units or override them", pick_list(l.labels(&[CadUnit::Auto, CadUnit::Points, CadUnit::Picas, CadUnit::Inches, CadUnit::Millimeters, CadUnit::Centimeters]), Some(l.label(d.settings.dxf_import)), |v| message(Action::DxfImport(v.value)))
                .width(174).text_size(BODY).padding(9).style(style::select).into(), Help::DxfImport), line(),
            setting(l, "DXF export units", "Coordinate units written into the file", pick_list(l.labels(&[CadUnit::Points, CadUnit::Picas, CadUnit::Inches, CadUnit::Millimeters, CadUnit::Centimeters]), Some(l.label(d.settings.dxf_export)), |v| message(Action::DxfExport(v.value)))
                .width(174).text_size(BODY).padding(9).style(style::select).into(), Help::DxfExport),
        ],
        Section::Cutting => column![
            toggle(l, "Merge shared lines", "Cut touching edges only once", d.settings.merge_lines, Action::Merge, Help::Merge), line(),
            numeric(l, d, Number::Time, "Cutting-time weight", "0 = material only · 1 = strongest cut preference", "0–1"),
            container(text(l.tr(if d.inputs[Number::Gap as usize].parse::<f64>().unwrap_or(1.) > 0. {
                "Your part gap is above zero. Set it to 0 in Layout to enable shared cuts. Spaced outlines stay separate."
            } else { "Shared-line export writes a cut network. Turn it off when a downstream tool requires separate closed part contours." }))
                .size(SMALL).color(MUTED_FOREGROUND)).padding(gap::SM).style(style::inset),
        ],
        Section::Search => {
            let fields = column![
                numeric(l, d, Number::Workers, "CPU workers", "1–8 layouts evaluated in parallel", "cores"), line(),
                numeric(l, d, Number::Population, "Population size", "Candidates kept in each generation", "layouts"), line(),
                numeric(l, d, Number::Mutation, "Mutation rate", "Chance of an order or rotation change", "%"), line(),
                toggle(l, "Search until stopped", "Keep improving the best layout", d.settings.continuous, Action::Continuous, Help::Budget),
            ];
            if d.settings.continuous { fields } else { fields.push(numeric(l, d, Number::Attempts, "Evaluation budget", "Stop after this many candidate layouts", "tries")) }
        }
    };
    let (title, explanation, tip) = help_copy(d.help);
    let guide = container(widgets::scroll_region(
        "settings-guide",
        column![
            text(l.tr("HOW IT WORKS"))
                .size(CAPTION)
                .font(MONO)
                .color(SUBTLE),
            canvas(Example(d, l)).width(Fill).height(182),
            text(l.tr(title)).size(HEADING).font(BOLD),
            text(l.tr(explanation)).size(BODY).color(FOREGROUND),
            text(l.tr(tip)).size(SMALL).color(MUTED_FOREGROUND),
        ]
        .spacing(gap::MD),
    ))
    .padding(gap::LG)
    .width(300)
    .height(Fill)
    .style(style::inset);
    let mut body = row![widgets::scroll_region("settings-fields", fields)].height(Fill);
    if window.width >= 800. {
        body = body.push(guide).spacing(gap::LG);
    }
    let content = container(
        column![
            row![
                column![
                    text(l.tr("Project settings")).size(TITLE).font(BOLD),
                    text(l.tr("Fine control, with a clear view of what each choice changes."))
                        .size(BODY)
                        .color(MUTED_FOREGROUND)
                ]
                .spacing(6),
                space::horizontal(),
                button(icon(Icon::Close, MUTED_FOREGROUND))
                    .padding(8)
                    .on_press(message(Action::Close))
                    .style(style::button_style(Button::Ghost))
            ]
            .align_y(Vertical::Center),
            tabs,
            line(),
            body,
            line(),
            text(d.error.as_ref().map_or_else(
                || {
                    l.tr("Settings are saved with this project. Apply changes when you’re ready.")
                        .to_owned()
                },
                |error| error.render(l),
            ))
            .size(SMALL)
            .color(if d.error.is_some() {
                DESTRUCTIVE
            } else {
                MUTED_FOREGROUND
            }),
            row![
                button(text(l.tr("Restore defaults")).size(BODY))
                    .padding([9, 12])
                    .on_press(message(Action::Reset))
                    .style(style::button_style(Button::Ghost)),
                space::horizontal(),
                button(text(l.tr("Cancel")).size(BODY))
                    .padding([9, 18])
                    .on_press(message(Action::Close))
                    .style(style::button_style(Button::Outline)),
                button(text(l.tr("Apply settings")).size(BODY).font(BOLD))
                    .padding([9, 18])
                    .on_press(message(Action::Apply))
                    .style(style::button_style(Button::Primary))
            ]
            .spacing(gap::XS),
        ]
        .spacing(gap::MD),
    )
    .padding(gap::LG)
    .width(936)
    .height((window.height - 64.).clamp(550., 720.))
    .style(style::dialog);
    container(opaque(content))
        .center_x(Fill)
        .center_y(Fill)
        .padding(gap::MD)
        .into()
}

fn help_copy(help: Help) -> (&'static str, &'static str, &'static str) {
    match help {
        Help::Units => ("Same sheet. Familiar units.", "Switch between millimeters and inches for dimensions, gaps, and rulers. A 25.4 mm part remains exactly one inch wide.", "This changes how measurements appear. Import and export units have their own controls."),
        Help::Gap => ("Leave room for the tool", "The gap is the shortest space between two cut outlines. Increase it for a wider tool, heat clearance, or easier part removal.", "Set the gap to zero when neighboring parts should share a laser cut. Stock edges receive the same clearance."),
        Help::Rotations => ("More angles, more possibilities", "4 orientations allow 0°, 90°, 180°, and 270°. With 8, the search can also use 45° steps. Use 1 when parts must keep their orientation.", "More angles help irregular shapes, but expand the search. Four is a useful starting point for rectangular parts."),
        Help::Placement => ("Choose what stays behind", "Gravity favors a narrow layout and a usable offcut. Bounding box reduces the enclosing rectangle. Compact reduces the enclosing convex area.", "Bounding box is a good default. Try Gravity when you want to save a strip of material for the next job."),
        Help::Curves => ("A curve becomes small lines", "Smaller tolerance follows curves more closely and uses more vertices. Larger tolerance reduces the work needed to calculate a layout.", "For example, 0.1 mm allows less deviation than 0.5 mm. Exported outlines use the same approximation. Reimport after changing it."),
        Help::Endpoints => ("Close tiny drafting gaps", "Nearly touching endpoints can be joined into a closed cutting contour. The tolerance is the largest gap the importer may repair.", "Start small. Increasing it can recover imperfect SVG or DXF files, but an excessive value may join edges that should stay apart. Ambiguous junctions are skipped."),
        Help::Simplify => ("A simpler envelope", "For nesting, a convex envelope surrounds each part. Fewer corners can speed up complex jobs, at the cost of less tightly packed material.", "The cut and exported outline stay intact. Repeated shapes are faster when imported once and assigned a quantity."),
        Help::Workers => ("Search several layouts at once", "Each worker evaluates a different candidate. More workers can finish a generation sooner, while using more CPU and memory.", "Try 2 on a laptop or 4 on a desktop. The interface remains responsive and Stop cancels all workers."),
        Help::Svg => ("Match the drawing’s scale", "A unitless SVG needs a conversion from drawing units to inches. At 96 units/inch, a 96-unit square measures 25.4 mm.", "Explicit physical dimensions and viewBox take priority. Choose 72, 90, 96, or your own value to match the source file."),
        Help::DxfImport => ("Read the drawing as intended", "Auto uses the DXF unit declaration. Select an override when a file is unitless or was saved with the wrong unit metadata.", "A one-unit line is 25.4 mm with Inches selected, or 1 mm with Millimeters. Auto treats unitless files as mm and reports that assumption."),
        Help::DxfExport => ("Speak your cutter’s units", "The layout keeps its physical size while coordinates convert to the chosen unit. Inches, mm, and cm also write the matching DXF unit declaration.", "Points and picas use unitless DXF coordinates because DXF has no code for those units. Use the matching import override if you bring that file back."),
        Help::Merge => ("One edge. One laser pass.", "When two parts touch, their common edge is exported once. Partial overlaps are combined too, reducing duplicate cuts and repeated heating.", "Requires zero part gap. SVG and DXF both support shared cuts. Use separate outlines when your downstream software expects closed individual parts."),
        Help::Time => ("Material versus cutting time", "At 0, placement focuses on material usage. Increasing the weight gives candidate positions with longer shared edges a better score.", "Use exact outlines, shared lines, and zero gap. Rough approximation skips contact scoring. This is a preference, not a machine-time estimate; try 0.5 first."),
        Help::Population => ("Keep more ideas in play", "Each generation contains several different part orders and rotations. The best candidate survives; successful candidates combine to produce the next generation.", "A small population moves quickly. A larger one explores more alternatives. Start at 10, then increase it for a difficult layout."),
        Help::Mutation => ("Explore beyond the current best", "Mutation changes neighboring part order and allowed rotations. A higher percentage introduces more variation into each generation.", "Try 10% first. Increase it when the search repeats similar arrangements; lower it to spend more effort refining successful ones."),
        Help::Budget => ("Stop when you have enough", "Search until stopped keeps producing generations. A fixed evaluation budget limits the number of candidate layouts to try.", "Use a budget larger than the population to explore several generations. Stop always keeps the best completed layout, ready to export."),
    }
}

struct Example<'a>(&'a Dialog, Language);
impl canvas::Program<Message> for Example<'_> {
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
        f.scale(bounds.width / 252.);
        let d = self.0;
        let l = self.1;
        let value =
            |n: Number, default: f64| d.inputs[n as usize].parse::<f64>().unwrap_or(default);
        let text = |f: &mut Frame, s: String, x, y| {
            f.fill_text(canvas::Text {
                content: s,
                position: Point::new(x, y),
                size: 10.0.into(),
                font: MONO,
                color: MUTED_FOREGROUND,
                ..Default::default()
            })
        };
        let rect = |f: &mut Frame, x, y, w, h, color| {
            let p =
                Path::rounded_rectangle(Point::new(x, y), Size::new(w, h), radius::BADGE.into());
            f.fill(&p, color);
            f.stroke(&p, Stroke::default().with_color(BORDER));
        };
        let line = |f: &mut Frame, a, b, color| {
            f.stroke(
                &Path::line(Point::from(a), Point::from(b)),
                Stroke::default().with_width(1.5).with_color(color),
            )
        };
        match d.help {
            Help::Gap | Help::Merge | Help::Time => {
                let merging = d.help != Help::Gap;
                let gap = if merging {
                    0.
                } else {
                    (value(Number::Gap, 3.) * d.settings.units.factor() * 4.).clamp(0., 44.) as f32
                };
                rect(&mut f, 20., 32., 92., 88., PARTS[0]);
                rect(&mut f, 112. + gap, 32., 92., 88., PARTS[1]);
                if merging {
                    line(&mut f, (112., 32.), (112., 120.), PRIMARY);
                } else {
                    line(&mut f, (114., 80.), (110. + gap, 80.), PRIMARY);
                }
                text(
                    &mut f,
                    if merging {
                        l.tr("2 parts / 1 shared cut").into()
                    } else {
                        trf!(
                            l,
                            "{gap} {unit} between outlines",
                            gap = d.inputs[0],
                            unit = d.settings.units.symbol()
                        )
                    },
                    20.,
                    142.,
                );
            }
            Help::Rotations => {
                let count = value(Number::Rotations, 4.).clamp(1., 32.) as usize;
                for i in 0..count {
                    let angle = i as f32 * std::f32::consts::TAU / count as f32;
                    let (s, c) = angle.sin_cos();
                    let center = Point::new(126. + c * 51., 77. + s * 51.);
                    let p = Path::new(|b| {
                        for (j, (x, y)) in
                            [(13., 0.), (-8., -8.), (-8., 8.)].into_iter().enumerate()
                        {
                            let pt = Point::new(center.x + x * c - y * s, center.y + x * s + y * c);
                            if j == 0 {
                                b.move_to(pt);
                            } else {
                                b.line_to(pt);
                            }
                        }
                        b.close();
                    });
                    f.fill(
                        &p,
                        if i == 0 {
                            PRIMARY
                        } else {
                            PARTS[i % PARTS.len()]
                        },
                    );
                }
                text(
                    &mut f,
                    trf!(
                        l,
                        "{count} orientations / {angle}° steps",
                        count = count,
                        angle = format!("{:.1}", 360. / count as f64)
                    ),
                    12.,
                    151.,
                );
            }
            Help::Placement => {
                for (i, title) in ["Gravity", "Box", "Compact"].into_iter().enumerate() {
                    let x = i as f32 * 85.;
                    rect(&mut f, x, 30., 78., 104., SHEET);
                    for j in 0..5 {
                        let (dx, dy) = match i {
                            0 => ((j % 2) * 23, (j / 2) * 27),
                            1 => ((j % 3) * 20, (j / 3) * 33),
                            _ => ((j % 3) * 19 + (j / 3) * 8, (j / 3) * 30 + ((j + 1) % 2) * 8),
                        };
                        rect(
                            &mut f,
                            x + 7. + dx as f32,
                            38. + dy as f32,
                            18.,
                            24.,
                            PARTS[j % PARTS.len()],
                        );
                    }
                    text(&mut f, l.tr(title).into(), x + 2., 145.);
                }
            }
            Help::Curves => {
                for (i, steps) in [4, 18].into_iter().enumerate() {
                    let x = 12. + i as f32 * 126.;
                    let curve = Path::new(|p| {
                        p.move_to(Point::new(x, 112.));
                        p.bezier_curve_to(
                            Point::new(x, 12.),
                            Point::new(x + 102., 12.),
                            Point::new(x + 102., 112.),
                        );
                    });
                    f.stroke(&curve, Stroke::default().with_color(SUBTLE).with_width(3.));
                    let p = Path::new(|p| {
                        for j in 0..=steps {
                            let t = j as f32 / steps as f32;
                            let a = Point::new(
                                x + 102. * (3. * t * t - 2. * t * t * t),
                                112. - 300. * t * (1. - t),
                            );
                            if j == 0 {
                                p.move_to(a);
                            } else {
                                p.line_to(a);
                            }
                        }
                    });
                    f.stroke(&p, Stroke::default().with_color(PRIMARY).with_width(1.5));
                }
                text(&mut f, l.tr("Faster").into(), 20., 142.);
                text(&mut f, l.tr("More precise").into(), 136., 142.);
            }
            Help::Endpoints => {
                line(&mut f, (22., 55.), (111., 87.), PRIMARY);
                line(&mut f, (125., 91.), (222., 49.), PRIMARY);
                for p in [Point::new(111., 87.), Point::new(125., 91.)] {
                    f.stroke(&Path::circle(p, 6.), Stroke::default().with_color(PRIMARY));
                }
                f.stroke(
                    &Path::circle(Point::new(118., 89.), 28.),
                    Stroke::default().with_color(SUBTLE),
                );
                text(&mut f, l.tr("Repair only small gaps").into(), 35., 143.);
            }
            Help::Simplify => {
                let hull = Path::new(|p| {
                    p.move_to(Point::new(45., 30.));
                    p.line_to(Point::new(195., 30.));
                    p.line_to(Point::new(195., 72.));
                    p.line_to(Point::new(90., 124.));
                    p.line_to(Point::new(45., 124.));
                    p.close();
                });
                f.fill(&hull, PRIMARY.scale_alpha(0.15));
                f.stroke(&hull, Stroke::default().with_color(PRIMARY));
                let outline = Path::new(|p| {
                    p.move_to(Point::new(45., 30.));
                    for (x, y) in [
                        (195., 30.),
                        (195., 72.),
                        (90., 72.),
                        (90., 124.),
                        (45., 124.),
                    ] {
                        p.line_to(Point::new(x, y));
                    }
                    p.close();
                });
                f.stroke(
                    &outline,
                    Stroke::default().with_color(FOREGROUND).with_width(2.),
                );
                text(&mut f, l.tr("Envelope / original cut").into(), 25., 145.);
            }
            Help::Workers => {
                let count = value(Number::Workers, 2.).clamp(1., 8.) as usize;
                for i in 0..8 {
                    rect(
                        &mut f,
                        20. + (i % 4) as f32 * 56.,
                        30. + (i / 4) as f32 * 49.,
                        44.,
                        34.,
                        if i < count { ACCENT } else { MUTED },
                    );
                }
                text(
                    &mut f,
                    trf!(l, "{count} layouts at a time", count = count),
                    34.,
                    145.,
                );
            }
            Help::Population | Help::Mutation => {
                for i in 0..12 {
                    let x = 12. + (i % 4) as f32 * 61.;
                    let y = 17. + (i / 4) as f32 * 37.;
                    rect(&mut f, x, y, 45., 27., MUTED);
                    for j in 0..3 {
                        rect(
                            &mut f,
                            x + 5. + j as f32 * 11.,
                            y + 5. + ((i + j) % 3) as f32 * 2.,
                            8.,
                            13.,
                            if i == 0 {
                                PRIMARY
                            } else {
                                PARTS[(i + j) % PARTS.len()]
                            },
                        );
                    }
                }
                text(
                    &mut f,
                    if d.help == Help::Mutation {
                        trf!(
                            l,
                            "{percent}% chance of a change",
                            percent = d.inputs[Number::Mutation as usize]
                        )
                    } else {
                        l.tr("Different layouts / one best").into()
                    },
                    5.,
                    145.,
                );
            }
            Help::Budget => {
                line(&mut f, (20., 125.), (232., 125.), BORDER);
                line(&mut f, (20., 125.), (20., 25.), BORDER);
                let points = [
                    (20., 112.),
                    (54., 91.),
                    (82., 67.),
                    (130., 52.),
                    (178., 44.),
                    (230., 42.),
                ];
                for pair in points.windows(2) {
                    line(&mut f, pair[0], pair[1], PRIMARY);
                }
                text(
                    &mut f,
                    l.tr("Keep the best. Stop anytime.").into(),
                    5.,
                    146.,
                );
            }
            Help::Units | Help::Svg | Help::DxfImport | Help::DxfExport => {
                rect(&mut f, 67., 24., 112., 100., SHEET);
                rect(&mut f, 84., 41., 78., 66., PARTS[1]);
                line(&mut f, (67., 137.), (179., 137.), PRIMARY);
                let caption = match d.help {
                    Help::Units => l.tr("25.4 mm = 1 inch").into(),
                    Help::Svg => trf!(
                        l,
                        "{count} units = 1 inch",
                        count = d.inputs[Number::Svg as usize]
                    ),
                    _ => l.tr("Same size / different units").into(),
                };
                text(&mut f, caption, 22., 152.);
            }
        }
        vec![f.into_geometry()]
    }
}
