use crate::{
    canvas::{Thumbnail, Workspace},
    icons::{icon, Icon},
    style::{self, Button},
    tokens::{color::*, layout, radius, space as gap, typography::*},
    widgets, App, ExportFormat, Message, StockField,
};
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, canvas, column, container, mouse_area, opaque, pick_list, progress_bar, row,
        scrollable, space, stack, text, text_input, Column,
    },
    Element, Fill, Theme,
};

fn action<'a>(
    label: &'a str,
    glyph: Icon,
    message: Option<Message>,
    kind: Button,
) -> iced::widget::Button<'a, Message> {
    let color = if message.is_none() {
        SUBTLE
    } else {
        match kind {
            Button::Primary => PRIMARY_FOREGROUND,
            Button::Danger => DESTRUCTIVE,
            _ => FOREGROUND,
        }
    };
    button(
        row![icon(glyph, color), text(label).size(BODY).font(BOLD)]
            .spacing(gap::XS)
            .align_y(Vertical::Center),
    )
    .padding([gap::XS, gap::SM])
    .height(layout::CONTROL)
    .on_press_maybe(message)
    .style(style::button_style(kind))
}
fn icon_button(
    glyph: Icon,
    hint: &'static str,
    message: Option<Message>,
    selected: bool,
) -> Element<'static, Message> {
    let color = if message.is_none() {
        SUBTLE
    } else if selected {
        PRIMARY
    } else {
        MUTED_FOREGROUND
    };
    widgets::hint(
        button(
            container(icon(glyph, color))
                .center_x(layout::CONTROL)
                .center_y(layout::CONTROL),
        )
        .padding(0)
        .on_press_maybe(message)
        .style(style::button_style(if selected {
            Button::Selected
        } else {
            Button::Ghost
        })),
        hint,
    )
}
fn divider() -> Element<'static, Message> {
    container(space::horizontal().height(1))
        .style(|_: &Theme| container::Style {
            background: Some(BORDER.into()),
            ..Default::default()
        })
        .into()
}
fn vertical_divider() -> Element<'static, Message> {
    container(space::vertical().width(1))
        .style(|_: &Theme| container::Style {
            background: Some(BORDER.into()),
            ..Default::default()
        })
        .into()
}
fn badge(label: String) -> Element<'static, Message> {
    container(text(label).size(CAPTION).font(MONO).color(MUTED_FOREGROUND))
        .padding([3., 6.])
        .style(|_: &Theme| container::Style {
            background: Some(MUTED.into()),
            border: iced::Border {
                radius: radius::SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
fn single_line(value: &str, length: usize) -> String {
    if value.chars().count() <= length {
        value.into()
    } else {
        format!("{}…", value.chars().take(length - 1).collect::<String>())
    }
}

pub fn workspace(app: &App) -> Element<'_, Message> {
    let menu = button(
        row![
            icon(Icon::Logo, PRIMARY).width(22).height(22),
            text("Deepnest").font(BOLD).size(HEADING),
            icon(Icon::ChevronDown, MUTED_FOREGROUND),
        ]
        .spacing(gap::SM)
        .align_y(Vertical::Center),
    )
    .padding([gap::XS, gap::XXS])
    .style(style::button_style(Button::Ghost))
    .on_press(Message::ToggleProjectMenu);
    let brand = container(menu).width(layout::SIDEBAR - gap::XL);
    let title = row![
        text(single_line(&app.project.name, 36))
            .size(BODY)
            .color(FOREGROUND),
        text(if app.dirty { "•" } else { "" })
            .size(BODY)
            .color(PRIMARY),
    ]
    .spacing(gap::XS)
    .align_y(Vertical::Center)
    .width(Fill);
    let run: Element<'_, Message> = if app.worker.is_some() {
        action("Stop", Icon::Stop, Some(Message::Stop), Button::Danger)
            .width(116)
            .into()
    } else {
        action(
            "Nest parts",
            Icon::Play,
            (app.project.quantity() > 0 && !app.busy_file).then_some(Message::Nest),
            Button::Primary,
        )
        .width(116)
        .into()
    };
    let toolbar = container(
        row![
            brand,
            title,
            icon_button(
                Icon::Settings,
                "Nesting settings · ⌘/Ctrl ,",
                Some(Message::Settings(crate::settings::Action::Open)),
                false
            ),
            icon_button(
                Icon::Save,
                "Save project · ⌘/Ctrl S",
                (!app.busy_file).then_some(Message::Save),
                false
            ),
            icon_button(
                Icon::Undo,
                "Undo · ⌘/Ctrl Z",
                (!app.history.is_empty()).then_some(Message::Undo),
                false
            ),
            container(vertical_divider())
                .height(20)
                .padding([0., gap::XXS]),
            action(
                "Import",
                Icon::Import,
                (!app.busy_file).then_some(Message::Import),
                Button::Ghost
            ),
            pick_list(
                [ExportFormat::Svg, ExportFormat::Dxf],
                Some(app.export_format),
                Message::ExportFormat
            )
            .text_size(SMALL)
            .padding([8, 8])
            .width(68)
            .style(style::select),
            action(
                "Export",
                Icon::Export,
                (app.layout.is_some() && app.worker.is_none() && !app.busy_file)
                    .then_some(Message::Export),
                Button::Outline
            ),
            run,
        ]
        .spacing(gap::XS)
        .align_y(Vertical::Center),
    )
    .padding([0., gap::MD])
    .height(layout::TOOLBAR)
    .align_y(Vertical::Center)
    .style(style::surface);

    let mut body = row![].height(Fill);
    if app.sidebar_progress.current > 0.0001 {
        body = body
            .push(widgets::reveal(
                library(app),
                app.sidebar_progress.current,
                true,
                layout::SIDEBAR,
            ))
            .push(vertical_divider());
    }
    body = body.push(drawing(app));
    let line: Element<'_, Message> = if app.worker.is_some() {
        progress_bar(0.0..=1.0, app.progress as f32)
            .girth(1)
            .style(|_| iced::widget::progress_bar::Style {
                background: BORDER.into(),
                bar: PRIMARY.into(),
                border: iced::Border::default(),
            })
            .into()
    } else {
        divider()
    };
    let base = column![toolbar, line, body, divider(), status(app)].height(Fill);
    let content: Element<'_, Message> = if app.menu_progress.current > 0.0001 {
        stack![
            base,
            mouse_area(container(space::horizontal()).height(Fill))
                .on_press(Message::ToggleProjectMenu),
            container(opaque(widgets::reveal(
                project_menu(app),
                app.menu_progress.current,
                false,
                200.
            )))
            .padding(iced::Padding {
                top: layout::TOOLBAR + gap::XXS,
                right: 0.,
                bottom: 0.,
                left: gap::SM
            }),
        ]
        .into()
    } else {
        base.into()
    };
    let content: Element<'_, Message> = if let Some(dialog) = &app.settings_dialog {
        stack![
            content,
            mouse_area(opaque(container(space::horizontal()).height(Fill).style(
                |_| container::Style {
                    background: Some(style::alpha(BACKGROUND, 0.72).into()),
                    ..Default::default()
                }
            )))
            .on_press(Message::Settings(crate::settings::Action::Close)),
            crate::settings_view::dialog(dialog, app.window_size),
        ]
        .into()
    } else {
        content
    };
    if app.tooltip.visible() {
        stack![
            content,
            canvas(crate::tooltip::Surface(&app.tooltip))
                .width(Fill)
                .height(Fill)
        ]
        .into()
    } else {
        content
    }
}

fn project_menu(app: &App) -> Element<'_, Message> {
    let available = !app.busy_file;
    let item = |label, glyph, message| {
        action(label, glyph, available.then_some(message), Button::Ghost).width(Fill)
    };
    container(
        column![
            item("New project", Icon::File, Message::New),
            item("Open project…", Icon::Folder, Message::Open),
            item("Save project…", Icon::Save, Message::Save),
            item(
                "Nesting settings…",
                Icon::Settings,
                Message::Settings(crate::settings::Action::Open)
            ),
            divider(),
            item("Load sample", Icon::Logo, Message::Demo),
        ]
        .spacing(gap::XXS),
    )
    .width(220)
    .padding(gap::XS)
    .style(style::floating)
    .into()
}

fn library(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            text("Parts").size(BODY).font(BOLD),
            badge(app.project.parts.len().to_string()),
            space::horizontal(),
            icon_button(
                Icon::Plus,
                "Import SVG or DXF",
                (!app.busy_file).then_some(Message::Import),
                false
            ),
        ]
        .spacing(gap::XS)
        .align_y(Vertical::Center),
    )
    .padding([gap::SM, gap::MD]);
    let mut parts = Column::new().spacing(gap::XXS);
    for (index, part) in app.project.parts.iter().enumerate() {
        let b = part.bounds();
        let selected = app.selected == Some(index);
        let thumb = container(canvas(Thumbnail { part, index }).width(32).height(32))
            .padding(gap::XXS)
            .style(|_: &Theme| container::Style {
                background: Some(INPUT.into()),
                border: iced::Border {
                    radius: radius::MD.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
        parts = parts.push(
            button(
                row![
                    thumb,
                    column![
                        text(single_line(&part.name, 20))
                            .size(BODY)
                            .color(if selected {
                                ACCENT_FOREGROUND
                            } else {
                                FOREGROUND
                            }),
                        text(format!(
                            "{} × {} {}",
                            app.project.settings.units.display(b.width),
                            app.project.settings.units.display(b.height),
                            app.project.settings.units.symbol()
                        ))
                        .size(CAPTION)
                        .color(MUTED_FOREGROUND),
                    ]
                    .spacing(gap::XXS)
                    .width(Fill),
                    text(format!("×{}", part.quantity))
                        .font(MONO)
                        .size(SMALL)
                        .color(if selected { PRIMARY } else { MUTED_FOREGROUND }),
                ]
                .spacing(gap::XS)
                .align_y(Vertical::Center),
            )
            .padding(gap::XS)
            .width(Fill)
            .style(style::button_style(if selected {
                Button::Selected
            } else {
                Button::Ghost
            }))
            .on_press(Message::SelectPart(index)),
        );
    }
    if app.project.parts.is_empty() {
        parts = parts.push(
            container(
                column![
                    text("No parts yet").size(BODY).color(FOREGROUND),
                    text("Import SVG or DXF files, or drop them into this window.")
                        .size(SMALL)
                        .color(MUTED_FOREGROUND),
                ]
                .spacing(gap::XS),
            )
            .padding(gap::XS),
        );
    }
    let list = scrollable(parts.padding([0., gap::XS]))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(3).scroller_width(3),
        ))
        .style(style::scroll)
        .height(Fill);
    let count = container(
        text(format!("{} copies in project", app.project.quantity()))
            .size(CAPTION)
            .color(SUBTLE),
    )
    .padding([gap::SM, gap::MD]);
    container(column![
        header,
        list,
        count,
        divider(),
        selection(app),
        divider(),
        stock(app)
    ])
    .style(style::surface)
    .width(layout::SIDEBAR)
    .height(Fill)
    .into()
}

fn selection(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = if let Some((index, part)) = app
        .selected
        .and_then(|index| app.project.parts.get(index).map(|part| (index, part)))
    {
        column![
            row![
                text("Selected part").size(SMALL).color(MUTED_FOREGROUND),
                space::horizontal(),
                icon_button(
                    Icon::Trash,
                    "Remove selected part",
                    Some(Message::RemovePart(index)),
                    false
                ),
            ]
            .align_y(Vertical::Center),
            text(single_line(&part.name, 30)).size(BODY).font(BOLD),
            row![
                text("Quantity").size(SMALL).color(MUTED_FOREGROUND),
                space::horizontal(),
                container(
                    row![
                        icon_button(
                            Icon::Minus,
                            "Decrease quantity",
                            (part.quantity > 0).then_some(Message::Quantity(index, -1)),
                            false
                        ),
                        text(part.quantity.to_string())
                            .font(MONO)
                            .size(BODY)
                            .width(30)
                            .align_x(text::Alignment::Center),
                        icon_button(
                            Icon::Plus,
                            "Increase quantity",
                            (part.quantity < 200).then_some(Message::Quantity(index, 1)),
                            false
                        ),
                    ]
                    .align_y(Vertical::Center)
                )
                .style(style::inset),
            ]
            .align_y(Vertical::Center),
        ]
        .spacing(gap::XXS)
        .into()
    } else {
        column![
            text("Selection").size(SMALL).color(MUTED_FOREGROUND),
            text("Select a part to change its quantity.")
                .size(BODY)
                .color(SUBTLE),
        ]
        .spacing(gap::SM)
        .into()
    };
    container(content)
        .padding([gap::SM, gap::MD])
        .height(128)
        .into()
}

fn stock(app: &App) -> Element<'_, Message> {
    let apply: Element<'_, Message> = if app.stock_pending() {
        button(text("Apply").size(SMALL).font(BOLD))
            .padding([gap::XXS, gap::XS])
            .style(style::button_style(Button::Selected))
            .on_press(Message::CommitStock)
            .into()
    } else {
        text(app.project.settings.units.symbol())
            .font(MONO)
            .size(CAPTION)
            .color(SUBTLE)
            .into()
    };
    let content = column![
        row![
            text("Stock sheet").size(BODY).font(BOLD),
            space::horizontal(),
            apply
        ]
        .height(26)
        .align_y(Vertical::Center),
        row![
            stock_input(app, StockField::Width, "Width"),
            stock_input(app, StockField::Height, "Height")
        ]
        .spacing(gap::SM),
        row![
            stock_input(app, StockField::Sheets, "Available sheets"),
            stock_input(app, StockField::Gap, "Part gap")
        ]
        .spacing(gap::SM),
        button(
            row![
                text("Nesting options").size(SMALL),
                space::horizontal(),
                icon(
                    if app.show_advanced {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    },
                    MUTED_FOREGROUND
                ),
            ]
            .align_y(Vertical::Center)
        )
        .padding([gap::XS, 0.])
        .width(Fill)
        .style(style::button_style(Button::Ghost))
        .on_press(Message::ToggleAdvanced),
    ]
    .spacing(gap::SM);
    let options = column![
        space::vertical().height(gap::SM),
        column![
            row![
                text("Rotations").size(SMALL).color(MUTED_FOREGROUND),
                space::horizontal(),
                pick_list(
                    (1..=32).collect::<Vec<_>>(),
                    Some(app.project.rotations),
                    Message::Rotations
                )
                .text_size(SMALL)
                .padding([6, 8])
                .width(76)
                .style(style::select)
            ]
            .align_y(Vertical::Center),
            row![
                text("Evaluations").size(SMALL).color(MUTED_FOREGROUND),
                space::horizontal(),
                pick_list(
                    [4, 8, 16, 32, 64, 128],
                    Some(app.project.attempts),
                    Message::Attempts
                )
                .text_size(SMALL)
                .padding([6, 8])
                .width(76)
                .style(style::select)
            ]
            .align_y(Vertical::Center),
        ]
        .spacing(gap::XS),
        button(
            row![
                icon(Icon::Settings, MUTED_FOREGROUND),
                text("All settings & guide").size(SMALL)
            ]
            .spacing(gap::XS)
        )
        .padding([gap::XS, 0.])
        .on_press(Message::Settings(crate::settings::Action::Open))
        .style(style::button_style(Button::Ghost)),
    ];
    container(column![
        content,
        widgets::reveal(options, app.options_progress.current, false, 140.)
    ])
    .padding([gap::SM, gap::MD])
    .into()
}
fn stock_input<'a>(app: &'a App, field: StockField, label: &'a str) -> Element<'a, Message> {
    column![
        text(label).size(CAPTION).color(MUTED_FOREGROUND),
        text_input("0", &app.stock_inputs[field as usize])
            .on_input_maybe(
                app.settings_dialog
                    .is_none()
                    .then_some(move |value| Message::StockInput(field, value))
            )
            .on_submit(Message::CommitStock)
            .font(MONO)
            .size(BODY)
            .padding([gap::XS, gap::SM])
            .style(style::input),
    ]
    .spacing(gap::XS)
    .width(Fill)
    .into()
}

fn drawing(app: &App) -> Element<'_, Message> {
    let drawing = canvas(Workspace(app)).width(Fill).height(Fill);
    let context: Element<'_, Message> = if let Some(layout) = &app.layout {
        row![
            icon(Icon::File, MUTED_FOREGROUND),
            text("Sheet").size(BODY).color(FOREGROUND),
            pick_list(
                (1..=layout.sheets.len()).collect::<Vec<_>>(),
                Some(app.sheet + 1),
                |i| Message::SelectSheet(i - 1)
            )
            .text_size(SMALL)
            .padding([5, 8])
            .width(62)
            .style(style::select),
            text(format!("of {}", layout.sheets.len()))
                .size(SMALL)
                .color(MUTED_FOREGROUND),
        ]
        .spacing(gap::XS)
        .align_y(Vertical::Center)
        .into()
    } else {
        row![
            icon(Icon::File, MUTED_FOREGROUND),
            text("Source parts").size(BODY).color(FOREGROUND),
            text("One of each shape").size(SMALL).color(SUBTLE)
        ]
        .spacing(gap::SM)
        .align_y(Vertical::Center)
        .into()
    };
    let top = row![
        icon_button(
            Icon::Sidebar,
            if app.show_sidebar {
                "Hide sidebar"
            } else {
                "Show sidebar"
            },
            Some(Message::ToggleSidebar),
            false
        ),
        context,
        space::horizontal(),
        text(format!(
            "{} × {} {}",
            app.project.settings.units.display(app.project.stock.width),
            app.project.settings.units.display(app.project.stock.height),
            app.project.settings.units.symbol(),
        ))
        .size(SMALL)
        .font(MONO)
        .color(MUTED_FOREGROUND),
    ]
    .spacing(gap::SM)
    .align_y(Vertical::Center);
    let summary = if let Some(result) = &app.layout {
        format!(
            "{} / {} placed    ·    {:.1}% material used",
            result.placed(),
            app.project.quantity(),
            result.utilization(&app.project)
        )
    } else {
        format!(
            "{} shapes    ·    {} copies",
            app.project.parts.len(),
            app.project.quantity()
        )
    };
    let zoom = container(
        row![
            icon_button(
                Icon::Minus,
                "Zoom out",
                Some(Message::Zoom(1. / 1.2)),
                false
            ),
            text(format!("{:.0}%", app.zoom.target * 100.))
                .size(SMALL)
                .font(MONO)
                .width(43)
                .align_x(text::Alignment::Center)
                .color(FOREGROUND),
            icon_button(Icon::Plus, "Zoom in", Some(Message::Zoom(1.2)), false),
            container(vertical_divider()).height(16),
            icon_button(Icon::Fit, "Fit sheet to view", Some(Message::Fit), false),
        ]
        .spacing(gap::XXS)
        .align_y(Vertical::Center),
    )
    .padding(gap::XXS)
    .style(style::floating);
    let bottom = row![
        text(summary).size(SMALL).color(MUTED_FOREGROUND),
        space::horizontal(),
        zoom,
        container(
            row![
                icon_button(
                    Icon::Grid,
                    "Toggle canvas grid",
                    Some(Message::ToggleGrid),
                    app.show_grid
                ),
                icon_button(
                    Icon::Help,
                    "Canvas controls and shortcuts",
                    Some(Message::ToggleHelp),
                    app.show_help
                ),
            ]
            .spacing(gap::XXS)
        )
        .padding(gap::XXS)
        .style(style::floating),
    ]
    .spacing(gap::SM)
    .align_y(Vertical::Center);
    let tools = container(column![top, space::vertical(), bottom])
        .padding(gap::LG)
        .height(Fill);
    let mut layers = stack![drawing, tools].width(Fill).height(Fill);
    if app.project.parts.is_empty() {
        let empty = column![
            icon(Icon::Import, SHEET_FOREGROUND).width(28).height(28),
            text("Make room for your ideas")
                .size(TITLE)
                .font(BOLD)
                .color(SHEET_FOREGROUND),
            text("Drop SVG or DXF files here to begin.")
                .size(BODY)
                .color(CUT_LINE),
            action(
                "Import parts",
                Icon::Plus,
                (!app.busy_file).then_some(Message::Import),
                Button::Primary
            ),
        ]
        .spacing(gap::MD)
        .align_x(Horizontal::Center);
        layers = layers.push(container(empty).center_x(Fill).center_y(Fill));
    }
    if app.show_help {
        let help = container(column![
            row![text("Canvas controls").font(BOLD).size(BODY),space::horizontal(),icon_button(Icon::Close,"Close help",Some(Message::ToggleHelp),false)].align_y(Vertical::Center),
            text("Scroll to zoom · Drag empty space to pan\nClick a part to select it").size(SMALL).color(MUTED_FOREGROUND),
            divider(),
            text("⌘/Ctrl + I    Import parts\n⌘/Ctrl + O    Open project\n⌘/Ctrl + S    Save project\n⌘/Ctrl + E    Export layout\n⌘/Ctrl + Z    Undo\n⌘/Ctrl + Enter    Nest parts\nEsc    Stop nesting / close menus").size(SMALL).font(MONO).color(MUTED_FOREGROUND),
        ].spacing(gap::SM)).padding(gap::MD).width(320).style(style::floating);
        layers = layers.push(
            container(column![
                space::vertical(),
                row![space::horizontal(), opaque(help)],
                space::vertical().height(64)
            ])
            .padding(gap::LG)
            .height(Fill),
        );
    }
    container(layers).height(Fill).width(Fill).clip(true).into()
}

fn status(app: &App) -> Element<'_, Message> {
    let color = if app.error {
        DESTRUCTIVE
    } else {
        MUTED_FOREGROUND
    };
    let activity = if app.worker.is_some() && app.project.settings.continuous {
        format!("Evaluation {} · searching until stopped", app.pass)
    } else if app.worker.is_some() {
        format!(
            "Evaluation {} / {} · {:.0}%",
            app.pass,
            app.project.attempts,
            app.progress * 100.
        )
    } else {
        app.layout
            .as_ref()
            .map_or("Local workspace".into(), |layout| {
                format!("Best evaluation {} · {:.1}s", layout.pass, layout.elapsed)
            })
    };
    container(
        row![
            text("•").size(BODY).color(if app.error {
                DESTRUCTIVE
            } else if app.worker.is_some() {
                PRIMARY
            } else {
                SUCCESS
            }),
            text(single_line(&app.status, 100))
                .size(CAPTION)
                .color(color)
                .width(Fill),
            text(activity).size(CAPTION).color(SUBTLE),
        ]
        .spacing(gap::XS)
        .align_y(Vertical::Center),
    )
    .padding([0., gap::MD])
    .height(layout::STATUS)
    .align_y(Vertical::Center)
    .style(style::surface)
    .into()
}
