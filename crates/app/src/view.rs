use crate::{
    canvas::{NestRing, SearchTrend, SheetPreview, Thumbnail, Workspace},
    icons::{icon, Icon},
    studio::{self, InspectorTab, LibraryTab},
    style::{self, Button},
    tokens::{color::*, layout, radius, space as gap, typography::*},
    widgets, windowing, App, ExportFormat, Message, StockField, WindowAction,
};
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, canvas, column, container, mouse_area, opaque, pick_list, progress_bar, row, space,
        stack, text, text_input, Column,
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
                radius: radius::BADGE.into(),
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

fn tab<'a>(label: &'a str, selected: bool, message: Message) -> Element<'a, Message> {
    button(container(text(label).size(SMALL).font(BOLD)).center_x(Fill))
        .padding([gap::XS, gap::SM])
        .width(Fill)
        .on_press(message)
        .style(style::button_style(if selected {
            Button::Selected
        } else {
            Button::Ghost
        }))
        .into()
}

fn section_title(label: &str) -> Element<'_, Message> {
    text(label).size(CAPTION).font(MONO).color(SUBTLE).into()
}

fn detail_row<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label).size(SMALL).color(MUTED_FOREGROUND),
        space::horizontal(),
        text(value).size(SMALL).color(FOREGROUND)
    ]
    .spacing(gap::XS)
    .align_y(Vertical::Center)
    .into()
}

fn library_tabs(app: &App) -> Element<'_, Message> {
    container(
        row![
            tab(
                "Parts",
                app.library_tab == LibraryTab::Parts,
                Message::LibraryTab(LibraryTab::Parts)
            ),
            tab(
                "Stock",
                app.library_tab == LibraryTab::Stock,
                Message::LibraryTab(LibraryTab::Stock)
            ),
        ]
        .spacing(gap::XXS),
    )
    .padding(gap::XXS)
    .style(style::inset)
    .into()
}

fn assets_visible(app: &App) -> bool {
    app.window_size.width >= 1000.
}

pub fn workspace(app: &App) -> Element<'_, Message> {
    let menu = button(
        row![
            text("Deepnest").font(BOLD).size(18),
            icon(Icon::ChevronDown, SUBTLE),
        ]
        .spacing(gap::SM)
        .align_y(Vertical::Center),
    )
    .padding([gap::XS, gap::XXS])
    .style(style::button_style(Button::Ghost))
    .on_press(Message::ToggleProjectMenu);
    let mut tools = row![
        container(icon(Icon::Logo, PRIMARY).width(24).height(24)).center_x(layout::RAIL),
        menu,
    ];
    if app.window_size.width >= 1000. {
        tools = tools
            .push(
                container(vertical_divider())
                    .height(18)
                    .padding([0., gap::XS]),
            )
            .push(
                text(single_line(&app.project.name, 28))
                    .size(BODY)
                    .color(MUTED_FOREGROUND),
            )
            .push(
                text(if app.dirty { "•" } else { "" })
                    .size(BODY)
                    .color(PRIMARY),
            );
    }
    tools = tools.push(space::horizontal());
    if app.window_size.width >= 1120. {
        tools = tools.push(
            text("NESTING STUDIO")
                .size(CAPTION)
                .font(MONO)
                .color(SUBTLE),
        );
    }
    // Import belongs to Job assets, with a fallback for compact windows.
    if !assets_visible(app) {
        tools = if app.window_size.width >= 800. {
            tools.push(action(
                "Import parts",
                Icon::Import,
                (!app.busy_file).then_some(Message::Import),
                Button::Outline,
            ))
        } else {
            tools.push(icon_button(
                Icon::Import,
                "Import SVG or DXF · Ctrl I",
                (!app.busy_file).then_some(Message::Import),
                false,
            ))
        };
    }
    if app.window_size.width < 800. {
        let running = app.worker.is_some();
        tools = tools.push(action(
            if running { "Stop" } else { "Nest" },
            if running { Icon::Stop } else { Icon::Play },
            if running {
                Some(Message::Stop)
            } else {
                (app.project.quantity() > 0 && !app.busy_file).then_some(Message::Nest)
            },
            if running {
                Button::Danger
            } else {
                Button::Primary
            },
        ));
    }
    let product_header = mouse_area(
        container(
            tools
                .spacing(gap::XS)
                .align_y(Vertical::Center)
                .height(Fill),
        )
        .padding(iced::Padding {
            right: gap::SM,
            ..Default::default()
        })
        .width(Fill)
        .height(Fill),
    )
    .on_press(Message::Window(WindowAction::Drag))
    .on_double_click(Message::Window(WindowAction::Maximize))
    .on_right_press(Message::Window(WindowAction::SystemMenu));
    let toolbar = row![product_header, window_controls(app)].height(layout::TOOLBAR);

    let mut body = row![navigation(app)].spacing(gap::XS).height(Fill);
    if assets_visible(app) {
        body = body.push(library(app));
    }
    body = body.push(drawing(app));
    if app.window_size.width >= 800. {
        body = body.push(inspector(app));
    }
    let body = container(body)
        .padding(iced::Padding {
            right: gap::SM,
            bottom: gap::XS,
            ..Default::default()
        })
        .height(Fill);
    let base = column![body, status(app)].height(Fill);
    let content: Element<'_, Message> = if app.menu_progress.current > 0.0001 {
        stack![
            base,
            mouse_area(container(space::horizontal()).height(Fill))
                .on_press(Message::ToggleProjectMenu),
            container(opaque(widgets::reveal(
                project_menu(app),
                app.menu_progress.current,
                false,
                164.
            )))
            .padding(iced::Padding {
                top: gap::XXS,
                right: 0.,
                bottom: 0.,
                left: layout::RAIL + gap::XS
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
            crate::settings_view::dialog(
                dialog,
                iced::Size::new(
                    app.window_size.width,
                    app.window_size.height - layout::TOOLBAR
                )
            ),
        ]
        .into()
    } else {
        content
    };
    let content: Element<'_, Message> = column![toolbar, content].height(Fill).into();
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

fn window_controls(app: &App) -> Element<'static, Message> {
    if !cfg!(target_os = "windows") {
        return space::horizontal().width(0).into();
    }
    let control = |glyph, action| {
        let is_close = matches!(action, WindowAction::Close);
        let is_maximize = matches!(action, WindowAction::Maximize);
        button(
            container(
                icon(glyph, FOREGROUND)
                    .width(layout::WINDOW_GLYPH)
                    .height(layout::WINDOW_GLYPH),
            )
            .center_x(Fill)
            .center_y(Fill),
        )
        .width(layout::WINDOW_CONTROL_WIDTH)
        .height(layout::WINDOW_TITLEBAR_HEIGHT)
        .padding(0)
        .on_press(Message::Window(action))
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered)
                || is_maximize && windowing::maximize_hovered();
            let background = if matches!(status, button::Status::Pressed) {
                Some(
                    if is_close {
                        WINDOW_CLOSE_PRESSED
                    } else {
                        MUTED
                    }
                    .into(),
                )
            } else if hovered {
                Some(if is_close { WINDOW_CLOSE_HOVER } else { MUTED }.into())
            } else {
                None
            };
            button::Style {
                background,
                text_color: FOREGROUND,
                border: iced::Border {
                    radius: radius::NONE.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
    };
    row![
        control(Icon::Minus, WindowAction::Minimize),
        control(
            if app.window_maximized {
                Icon::Restore
            } else {
                Icon::Window
            },
            WindowAction::Maximize
        ),
        control(Icon::Close, WindowAction::Close),
    ]
    .spacing(0)
    .height(layout::WINDOW_TITLEBAR_HEIGHT)
    .into()
}

fn project_menu(app: &App) -> Element<'_, Message> {
    let available = !app.busy_file;
    let item = |label, glyph, shortcut, message| {
        button(
            row![
                icon(glyph, MUTED_FOREGROUND),
                text(label).size(BODY),
                space::horizontal(),
                text(shortcut).size(CAPTION).font(MONO).color(SUBTLE),
            ]
            .spacing(gap::XS)
            .align_y(Vertical::Center),
        )
        .padding([gap::XS, gap::SM])
        .height(layout::CONTROL)
        .on_press_maybe(available.then_some(message))
        .style(style::button_style(Button::Ghost))
        .width(Fill)
    };
    container(
        column![
            item("New project", Icon::File, "", Message::New),
            item("Open project…", Icon::Folder, "Ctrl O", Message::Open),
            item("Save project…", Icon::Save, "Ctrl S", Message::Save),
            divider(),
            item("Load sample", Icon::Logo, "", Message::Demo),
        ]
        .spacing(gap::XXS),
    )
    .width(248)
    .padding(gap::XS)
    .style(style::floating)
    .into()
}

fn navigation(app: &App) -> Element<'_, Message> {
    container(
        column![
            icon_button(
                Icon::Undo,
                "Undo · Ctrl Z",
                (!app.history.is_empty()).then_some(Message::Undo),
                false
            ),
            space::vertical(),
            icon_button(
                Icon::Settings,
                "Project settings · Ctrl ,",
                Some(Message::Settings(crate::settings::Action::Open)),
                app.settings_dialog.is_some()
            ),
        ]
        .spacing(gap::SM)
        .align_x(Horizontal::Center)
        .height(Fill),
    )
    .padding([gap::SM, gap::XS])
    .width(layout::RAIL)
    .height(Fill)
    .into()
}

fn library(app: &App) -> Element<'_, Message> {
    if app.library_tab == LibraryTab::Stock {
        return stock_library(app);
    }
    let header = column![
        row![
            text("Job assets").size(HEADING).font(BOLD),
            space::horizontal(),
            badge(format!("{:02}", app.project.parts.len())),
        ]
        .align_y(Vertical::Center),
        library_tabs(app),
        row![
            icon(Icon::Search, SUBTLE),
            text_input("Find a part…", &app.part_filter)
                .on_input(Message::FilterParts)
                .size(SMALL)
                .padding(gap::XS)
                .style(style::input)
        ]
        .spacing(gap::XS)
        .align_y(Vertical::Center),
        action(
            "Import SVG / DXF",
            Icon::Plus,
            (!app.busy_file).then_some(Message::Import),
            Button::Outline
        )
        .width(Fill),
        row![
            text("PROJECT PARTS").size(CAPTION).font(MONO).color(SUBTLE),
            space::horizontal(),
            text(format!("{} copies", app.project.quantity()))
                .size(CAPTION)
                .color(MUTED_FOREGROUND),
        ],
    ]
    .spacing(gap::MD);
    let mut parts = Column::new().spacing(gap::XS);
    let query = app.part_filter.to_lowercase();
    let mut visible_parts = 0;
    for (index, part) in app.project.parts.iter().enumerate() {
        if !part.name.to_lowercase().contains(&query) {
            continue;
        }
        visible_parts += 1;
        let b = part.bounds();
        let selected = app.selected == Some(index);
        let thumb = container(canvas(Thumbnail { part, index }).width(34).height(34))
            .padding(5)
            .style(style::inset);
        parts = parts.push(
            button(
                row![
                    thumb,
                    column![
                        text(single_line(&part.name, 19))
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
                    .spacing(5)
                    .width(Fill),
                    text(format!("×{}", part.quantity))
                        .font(MONO)
                        .size(SMALL)
                        .color(if selected { PRIMARY } else { SUBTLE }),
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
    if visible_parts == 0 && !app.project.parts.is_empty() {
        parts = parts
            .push(container(text("No matching parts").size(SMALL).color(SUBTLE)).padding(gap::SM));
    }
    if app.project.parts.is_empty() {
        parts = parts.push(
            container(
                column![
                    text("No parts yet").size(BODY).color(FOREGROUND),
                    text("Imported shapes will appear here.")
                        .size(SMALL)
                        .color(MUTED_FOREGROUND),
                ]
                .spacing(gap::SM),
            )
            .padding(gap::SM),
        );
    }
    let list = widgets::scroll_region("part-library", parts);
    let detail = container(selection(app)).style(style::card);
    widgets::panel(
        container(
            column![
                header,
                list,
                detail,
                row![
                    icon(Icon::File, SUBTLE),
                    text("Original contours preserved")
                        .size(CAPTION)
                        .color(SUBTLE),
                ]
                .spacing(gap::XS)
                .align_y(Vertical::Center),
            ]
            .spacing(gap::MD)
            .height(Fill),
        )
        .padding(gap::SM)
        .width(layout::SIDEBAR)
        .height(Fill),
    )
}

fn stock_library(app: &App) -> Element<'_, Message> {
    let stock_card = container(
        column![
            row![
                icon(Icon::Layers, PRIMARY),
                text("Sheet preview").size(BODY).font(BOLD),
            ]
            .spacing(gap::XS)
            .align_y(Vertical::Center),
            canvas(SheetPreview {
                project: &app.project,
                placements: &[]
            })
            .width(Fill)
            .height(112),
        ]
        .spacing(gap::SM),
    )
    .padding(gap::SM)
    .style(style::card);
    widgets::panel(
        container(
            column![
                row![
                    text("Job assets").size(HEADING).font(BOLD),
                    space::horizontal(),
                    badge("01".into())
                ]
                .align_y(Vertical::Center),
                library_tabs(app),
                widgets::scroll_region(
                    "stock-library",
                    column![
                        stock_card,
                        stock(app),
                        text("Sheet size and spacing apply to every sheet in this job.")
                            .size(SMALL)
                            .color(SUBTLE)
                    ]
                    .spacing(gap::MD)
                ),
            ]
            .spacing(gap::MD)
            .height(Fill),
        )
        .padding(gap::SM)
        .width(layout::SIDEBAR)
        .height(Fill),
    )
}

fn inspector(app: &App) -> Element<'_, Message> {
    let running = app.worker.is_some();
    let ready = app.layout.is_some();
    let control = button(
        container(
            column![
                icon(
                    if running { Icon::Stop } else { Icon::Play },
                    if running {
                        DESTRUCTIVE
                    } else {
                        PRIMARY_FOREGROUND
                    }
                )
                .width(20)
                .height(20),
                text(if running { "Stop" } else { "Nest" })
                    .size(SMALL)
                    .font(BOLD),
            ]
            .spacing(5)
            .align_x(Horizontal::Center),
        )
        .center_x(76)
        .center_y(76),
    )
    .width(76)
    .height(76)
    .padding(0)
    .on_press_maybe(if running {
        Some(Message::Stop)
    } else {
        (app.project.quantity() > 0 && !app.busy_file).then_some(Message::Nest)
    })
    .style(move |theme, status| {
        let mut style = style::button_style(if running {
            Button::Danger
        } else {
            Button::Primary
        })(theme, status);
        style.border.radius = radius::circle(76.).into();
        style
    });
    let dial = stack![
        canvas(NestRing {
            progress: app.nest_progress.current
        })
        .width(128)
        .height(128),
        container(control).center_x(128).center_y(128),
    ];
    let phase = if running && app.run.stats.preparation_seconds == 0. {
        "Preparing geometry"
    } else if running {
        "Searching layouts"
    } else if ready {
        "Layout ready"
    } else {
        "Ready to optimize"
    };
    let run_label = if running && app.project.settings.continuous {
        format!("Evaluation {}", app.pass)
    } else if running {
        format!("{} / {} evaluations", app.pass, app.project.attempts)
    } else if app.run.stats.evaluations > 0 {
        format!("{} evaluated", app.run.stats.evaluations)
    } else {
        format!("{} parts in this job", app.project.quantity())
    };
    let header = column![
        text("Optimizer").size(18).font(BOLD),
        row![
            badge(format!("CPU · {} workers", app.project.settings.workers)),
            space::horizontal(),
            text("LOCAL ENGINE").size(CAPTION).font(MONO).color(SUBTLE)
        ]
        .align_y(Vertical::Center),
        row![
            dial,
            column![
                text(phase)
                    .size(BODY)
                    .font(BOLD)
                    .color(if running { PRIMARY } else { FOREGROUND }),
                text(studio::duration(app.run.stats.elapsed))
                    .font(MONO)
                    .size(24)
                    .color(FOREGROUND),
                text(run_label).size(CAPTION).color(MUTED_FOREGROUND),
            ]
            .spacing(gap::XS)
            .width(Fill)
        ]
        .spacing(gap::XXS)
        .align_y(Vertical::Center),
    ]
    .spacing(gap::XS);
    let metric = |label: &'static str, value: String| {
        column![
            text(value).size(HEADING).font(MONO).color(FOREGROUND),
            text(label).size(CAPTION).color(MUTED_FOREGROUND),
        ]
        .spacing(4)
        .width(Fill)
    };
    let utilization = app
        .layout
        .as_ref()
        .map_or(0., |r| r.utilization(&app.project));
    let placed = app.layout.as_ref().map_or(0, |r| r.placed());
    let sheets = app.layout.as_ref().map_or(0, |r| r.sheets.len());
    let stats = container(
        column![
            row![
                column![
                    text("MATERIAL USE")
                        .size(CAPTION)
                        .font(MONO)
                        .color(MUTED_FOREGROUND),
                    text(if ready {
                        format!("{utilization:.1}%")
                    } else {
                        "—".into()
                    })
                    .size(32)
                    .color(FOREGROUND)
                ]
                .spacing(4),
                space::horizontal(),
                icon(Icon::Chart, PRIMARY).width(24).height(24),
            ]
            .align_y(Vertical::Center),
            progress_bar(0.0..=100.0, utilization as f32)
                .girth(4)
                .style(|_| iced::widget::progress_bar::Style {
                    background: MUTED.into(),
                    bar: PRIMARY.into(),
                    border: iced::Border {
                        radius: radius::BADGE.into(),
                        ..Default::default()
                    },
                }),
            row![
                metric(
                    "Parts placed",
                    format!("{placed} / {}", app.project.quantity())
                ),
                metric(
                    "Sheets used",
                    format!("{sheets} / {}", app.project.stock.sheets)
                )
            ]
            .spacing(gap::XS),
        ]
        .spacing(gap::SM),
    )
    .padding(gap::MD)
    .style(style::card);
    let tabs = container(
        row![
            tab(
                "Setup",
                app.inspector_tab == InspectorTab::Setup,
                Message::InspectorTab(InspectorTab::Setup)
            ),
            tab(
                "Run insights",
                app.inspector_tab == InspectorTab::Insights,
                Message::InspectorTab(InspectorTab::Insights)
            ),
        ]
        .spacing(gap::XXS),
    )
    .padding(gap::XXS)
    .style(style::inset);
    let content = column![
        header,
        stats,
        tabs,
        match app.inspector_tab {
            InspectorTab::Setup => optimizer_setup(app),
            InspectorTab::Insights => run_insights(app),
        }
    ]
    .spacing(gap::MD);
    let exportable = ready && !running && !app.busy_file && !app.stock_pending();
    let (label, tint) = if app.stock_pending() {
        ("Stock changes pending".into(), WARNING)
    } else if ready && placed < app.project.quantity() {
        (
            format!("{} parts unplaced", app.project.quantity() - placed),
            WARNING,
        )
    } else if ready {
        ("Contours verified".into(), SUCCESS)
    } else {
        ("Run nesting to create a layout".into(), SUBTLE)
    };
    let export = container(
        column![
            row![
                icon(if ready { Icon::Check } else { Icon::File }, tint),
                text(label).size(CAPTION).color(tint)
            ]
            .spacing(gap::XS)
            .align_y(Vertical::Center),
            row![
                action(
                    "Export layout",
                    Icon::Export,
                    exportable.then_some(Message::Export),
                    if exportable {
                        Button::Primary
                    } else {
                        Button::Outline
                    }
                )
                .width(Fill),
                pick_list(
                    [ExportFormat::Svg, ExportFormat::Dxf],
                    Some(app.export_format),
                    Message::ExportFormat
                )
                .text_size(SMALL)
                .padding([8, 8])
                .width(68)
                .style(style::select),
            ]
            .spacing(gap::XS)
            .align_y(Vertical::Center),
        ]
        .spacing(gap::SM),
    )
    .padding(gap::MD);
    widgets::panel(
        container(column![
            widgets::scroll_region(
                "nesting-inspector",
                content.padding(iced::Padding {
                    top: gap::XS,
                    right: 0.,
                    bottom: gap::XS,
                    left: gap::MD
                })
            ),
            divider(),
            export,
        ])
        .width(layout::INSPECTOR)
        .height(Fill),
    )
}

fn optimizer_setup(app: &App) -> Element<'_, Message> {
    let running = app.worker.is_some();
    let mut presets = iced::widget::Row::new().spacing(gap::XXS);
    for (label, attempts) in [("Quick", 16), ("Balanced", 64), ("Deep", 256)] {
        let selected = !app.project.settings.continuous && app.project.attempts == attempts;
        presets = presets.push(
            button(container(text(label).size(SMALL)).center_x(Fill))
                .width(Fill)
                .padding([gap::XS, 4.])
                .on_press_maybe((!running).then_some(Message::Attempts(attempts)))
                .style(style::button_style(if selected {
                    Button::Selected
                } else {
                    Button::Outline
                })),
        );
    }
    let strategy: Element<'_, Message> = if running {
        badge(app.project.settings.placement.to_string())
    } else {
        pick_list(
            [
                crate::settings::Placement::BoundingBox,
                crate::settings::Placement::Gravity,
                crate::settings::Placement::Compact,
            ],
            Some(app.project.settings.placement),
            Message::Placement,
        )
        .text_size(SMALL)
        .padding([7, 8])
        .width(130)
        .style(style::select)
        .into()
    };
    let rotations: Element<'_, Message> = if running {
        badge(app.project.rotations.to_string())
    } else {
        pick_list(
            (1..=32).collect::<Vec<_>>(),
            Some(app.project.rotations),
            Message::Rotations,
        )
        .text_size(SMALL)
        .padding([7, 8])
        .width(80)
        .style(style::select)
        .into()
    };
    let mut content = column![
        section_title("SEARCH BUDGET"),
        presets,
        button(
            row![
                text("Search until stopped").size(SMALL),
                space::horizontal(),
                icon(
                    if app.project.settings.continuous {
                        Icon::Check
                    } else {
                        Icon::Plus
                    },
                    if app.project.settings.continuous {
                        PRIMARY
                    } else {
                        SUBTLE
                    }
                ),
            ]
            .align_y(Vertical::Center)
        )
        .width(Fill)
        .padding([gap::XS, gap::SM])
        .on_press_maybe((!running).then_some(Message::Continuous(!app.project.settings.continuous)))
        .style(style::button_style(if app.project.settings.continuous {
            Button::Selected
        } else {
            Button::Ghost
        })),
        text(if app.project.settings.continuous {
            "Refines the best layout until you stop.".into()
        } else {
            format!("Up to {} candidate layouts per run.", app.project.attempts)
        })
        .size(CAPTION)
        .color(SUBTLE),
        divider(),
        section_title("PLACEMENT"),
        row![
            text("Strategy").size(SMALL).color(MUTED_FOREGROUND),
            space::horizontal(),
            strategy
        ]
        .align_y(Vertical::Center),
        row![
            text("Rotation steps").size(SMALL).color(MUTED_FOREGROUND),
            space::horizontal(),
            rotations
        ]
        .align_y(Vertical::Center),
    ]
    .spacing(gap::SM);
    if !assets_visible(app) {
        content = content.push(divider()).push(stock(app));
    }
    content.into()
}

fn run_insights(app: &App) -> Element<'_, Message> {
    let stats = &app.run.stats;
    let queries = stats.cache_hits + stats.cache_misses;
    let reuse = if queries == 0 {
        "—".into()
    } else {
        format!("{:.1}%", stats.cache_hits as f64 / queries as f64 * 100.)
    };
    let mut content = column![
        section_title("MATERIAL USE OVER TIME"),
        container(
            column![
                canvas(SearchTrend {
                    samples: &app.run.history,
                    elapsed: stats.elapsed
                })
                .width(Fill)
                .height(92),
                row![
                    text("0s").size(CAPTION).font(MONO).color(SUBTLE),
                    space::horizontal(),
                    text(studio::duration(stats.elapsed))
                        .size(CAPTION)
                        .font(MONO)
                        .color(SUBTLE)
                ],
            ]
            .spacing(gap::XS)
        )
        .padding(gap::XS)
        .style(style::inset),
    ]
    .spacing(gap::SM);
    if app.run.history.is_empty() {
        content = content.push(
            text("Start a run to see verified improvements.")
                .size(CAPTION)
                .color(SUBTLE),
        );
    }
    content.push(section_title("RUN PERFORMANCE")).push(container(column![
        detail_row("Evaluations", stats.evaluations.to_string()),
        detail_row("Valid candidates", stats.evaluations.saturating_sub(stats.rejected).to_string()),
        detail_row("Best updates", stats.improvements.to_string()),
        divider(),
        detail_row("First valid layout", stats.first_layout_seconds.map_or("—".into(), studio::duration)),
        detail_row("Geometry preparation", studio::duration(stats.preparation_seconds)),
        detail_row("Validation", studio::duration(stats.validation_seconds)),
        detail_row("Geometry cache reuse", reuse),
    ].spacing(gap::SM)).padding(gap::SM).style(style::card))
    .push(text("Every accepted layout is checked against the original contours and clearance rules.").size(SMALL).color(MUTED_FOREGROUND)).into()
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
        .height(138)
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
    ]
    .spacing(gap::SM);
    container(content).into()
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
            text(format!("Sheet {:02}", app.sheet + 1))
                .size(BODY)
                .color(FOREGROUND),
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
            text("Source preview").size(BODY).color(FOREGROUND)
        ]
        .spacing(gap::SM)
        .align_y(Vertical::Center)
        .into()
    };
    let top = column![
        row![
            section_title("LAYOUT WORKSPACE"),
            space::horizontal(),
            text(if app.stock_pending() {
                "CHANGES PENDING"
            } else if app.layout.is_some() {
                "VERIFIED LAYOUT"
            } else {
                "SOURCE GEOMETRY"
            })
            .size(CAPTION)
            .font(MONO)
            .color(if app.stock_pending() {
                WARNING
            } else if app.layout.is_some() {
                SUCCESS
            } else {
                SUBTLE
            })
        ]
        .spacing(gap::XS),
        row![
            context,
            space::horizontal(),
            text(format!(
                "{} × {} {}",
                app.project.settings.units.display(app.project.stock.width),
                app.project.settings.units.display(app.project.stock.height),
                app.project.settings.units.symbol()
            ))
            .size(CAPTION)
            .font(MONO)
            .color(SUBTLE)
        ]
        .align_y(Vertical::Center),
    ]
    .spacing(gap::XS);
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
    let zoom = widgets::tool_group(
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
    );
    let bottom = row![
        zoom,
        space::horizontal(),
        widgets::tool_group(
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
        ),
    ]
    .spacing(gap::SM)
    .align_y(Vertical::Center);
    // Aggregate results belong to the optimizer; keep them here only in compact mode.
    let bottom: Element<'_, Message> = if app.window_size.width < 800. {
        column![text(summary).size(SMALL).color(MUTED_FOREGROUND), bottom]
            .spacing(gap::SM)
            .into()
    } else {
        bottom.into()
    };
    let tools = container(column![space::vertical(), bottom])
        .padding(gap::MD)
        .height(Fill);
    let mut layers = stack![drawing, tools].width(Fill).height(Fill);
    if app.project.parts.is_empty() {
        let empty = column![
            icon(Icon::Import, SHEET_FOREGROUND).width(28).height(28),
            text("Build your first cutting job")
                .size(TITLE)
                .font(BOLD)
                .color(SHEET_FOREGROUND),
            text("Drop SVG or DXF files here to begin.")
                .size(BODY)
                .color(CUT_LINE),
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
    widgets::panel(
        container(column![
            container(top).padding([gap::MD, gap::LG]),
            divider(),
            layers,
            divider(),
            sheet_tray(app),
        ])
        .height(Fill)
        .width(Fill),
    )
}

fn sheet_tray(app: &App) -> Element<'_, Message> {
    let Some(result) = &app.layout else {
        return container(
            row![
                icon(Icon::Layers, SUBTLE),
                column![
                    text("Your sheets, at a glance")
                        .size(SMALL)
                        .color(MUTED_FOREGROUND),
                    text("Run nesting to compare material use across sheets.")
                        .size(CAPTION)
                        .color(SUBTLE),
                ]
                .spacing(4)
            ]
            .spacing(gap::SM)
            .align_y(Vertical::Center),
        )
        .padding([gap::SM, gap::LG])
        .height(86)
        .align_y(Vertical::Center)
        .into();
    };
    let jump_to_sheet = result.sheets.len() > 3;
    let available = app.window_size.width
        - layout::RAIL
        - if app.window_size.width >= 800. {
            layout::INSPECTOR
        } else {
            0.
        }
        - if assets_visible(app) {
            layout::SIDEBAR
        } else {
            0.
        }
        - if jump_to_sheet { 78. } else { 0. };
    let count = if available >= 620. {
        3
    } else if available >= 430. {
        2
    } else {
        1
    };
    let start = app
        .sheet
        .saturating_sub(count / 2)
        .min(result.sheets.len().saturating_sub(count));
    let mut cards = iced::widget::Row::new().spacing(gap::XS);
    for index in start..(start + count).min(result.sheets.len()) {
        let selected = index == app.sheet;
        cards = cards.push(
            button(
                row![
                    canvas(SheetPreview {
                        project: &app.project,
                        placements: &result.sheets[index]
                    })
                    .width(74)
                    .height(48),
                    column![
                        text(format!("Sheet {:02}", index + 1))
                            .size(SMALL)
                            .font(BOLD)
                            .color(if selected {
                                ACCENT_FOREGROUND
                            } else {
                                FOREGROUND
                            }),
                        text(format!(
                            "{:.1}% · {} parts",
                            result.sheet_utilization(&app.project, index),
                            result.sheets[index].len()
                        ))
                        .size(CAPTION)
                        .color(MUTED_FOREGROUND),
                    ]
                    .spacing(5)
                    .width(Fill),
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
            .on_press(Message::SelectSheet(index)),
        );
    }
    let mut navigation = row![].spacing(gap::XXS).align_y(Vertical::Center);
    if jump_to_sheet {
        navigation = navigation.push(widgets::hint(
            pick_list(
                (1..=result.sheets.len()).collect::<Vec<_>>(),
                Some(app.sheet + 1),
                |i| Message::SelectSheet(i - 1),
            )
            .text_size(SMALL)
            .padding([8, 8])
            .width(70)
            .style(style::select),
            "Jump to sheet",
        ));
    }
    container(
        navigation
            .push(icon_button(
                Icon::ChevronLeft,
                "Previous sheet",
                (app.sheet > 0).then(|| Message::SelectSheet(app.sheet - 1)),
                false,
            ))
            .push(cards.width(Fill))
            .push(icon_button(
                Icon::ChevronRight,
                "Next sheet",
                (app.sheet + 1 < result.sheets.len())
                    .then_some(Message::SelectSheet(app.sheet + 1)),
                false,
            )),
    )
    .padding([gap::XS, gap::SM])
    .height(86)
    .into()
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
    .into()
}
