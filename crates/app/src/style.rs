//! Reusable component variants built exclusively from semantic design tokens.
use crate::tokens::{color::*, radius, scrollbar};
use iced::{
    widget::{button, container, pick_list, scrollable, text_input},
    Border, Color, Shadow, Theme, Vector,
};

pub fn alpha(color: Color, opacity: f32) -> Color {
    color.scale_alpha(opacity)
}
pub fn border(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: radius.into(),
    }
}
pub fn floating(_: &Theme) -> container::Style {
    container::Style {
        background: Some(POPOVER.into()),
        text_color: Some(FOREGROUND),
        border: border(BORDER, radius::POPOVER),
        shadow: Shadow {
            color: SHADOW,
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    }
}
pub fn tool_group(theme: &Theme) -> container::Style {
    container::Style {
        border: border(BORDER, radius::TOOL_GROUP),
        ..floating(theme)
    }
}
pub fn dialog(theme: &Theme) -> container::Style {
    container::Style {
        border: border(BORDER, radius::PANEL),
        ..floating(theme)
    }
}
pub fn panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SIDEBAR.into()),
        text_color: Some(FOREGROUND),
        border: border(alpha(BORDER, 0.65), radius::PANEL),
        shadow: Shadow {
            color: alpha(SHADOW, 0.5),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    }
}
pub fn card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(POPOVER.into()),
        text_color: Some(FOREGROUND),
        border: border(alpha(BORDER, 0.55), radius::CARD),
        ..Default::default()
    }
}
pub fn inset(_: &Theme) -> container::Style {
    container::Style {
        background: Some(INPUT.into()),
        border: border(BORDER, radius::CONTROL),
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
pub enum Button {
    Primary,
    Outline,
    Ghost,
    Selected,
    Danger,
}

pub fn button_style(kind: Button) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (mut background, foreground, line) = match kind {
            Button::Primary => (PRIMARY, PRIMARY_FOREGROUND, Color::TRANSPARENT),
            Button::Outline => (Color::TRANSPARENT, FOREGROUND, BORDER),
            Button::Ghost => (Color::TRANSPARENT, MUTED_FOREGROUND, Color::TRANSPARENT),
            Button::Selected => (ACCENT, ACCENT_FOREGROUND, alpha(PRIMARY, 0.22)),
            Button::Danger => (
                alpha(DESTRUCTIVE, 0.10),
                DESTRUCTIVE,
                alpha(DESTRUCTIVE, 0.20),
            ),
        };
        match status {
            button::Status::Hovered => {
                background = match kind {
                    Button::Primary => blend(PRIMARY, FOREGROUND, 0.12),
                    Button::Selected => blend(ACCENT, PRIMARY, 0.08),
                    Button::Danger => alpha(DESTRUCTIVE, 0.17),
                    _ => MUTED,
                }
            }
            button::Status::Pressed => {
                background = match kind {
                    Button::Primary => blend(PRIMARY, PRIMARY_FOREGROUND, 0.14),
                    _ => ACCENT,
                }
            }
            _ => (),
        }
        let disabled = matches!(status, button::Status::Disabled);
        button::Style {
            background: Some(
                if disabled {
                    alpha(background, 0.35)
                } else {
                    background
                }
                .into(),
            ),
            text_color: if disabled {
                alpha(foreground, 0.35)
            } else {
                foreground
            },
            border: border(
                if disabled { alpha(line, 0.4) } else { line },
                radius::CONTROL,
            ),
            shadow: if matches!(kind, Button::Primary) && !disabled {
                Shadow {
                    color: alpha(PRIMARY, 0.12),
                    offset: Vector::new(0.0, 3.0),
                    blur_radius: 14.0,
                }
            } else {
                Shadow::default()
            },
            ..Default::default()
        }
    }
}
fn blend(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}
pub fn input(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: INPUT.into(),
        border: border(
            if matches!(status, text_input::Status::Focused { .. }) {
                RING
            } else {
                BORDER
            },
            radius::CONTROL,
        ),
        icon: MUTED_FOREGROUND,
        placeholder: SUBTLE,
        value: FOREGROUND,
        selection: ACCENT,
    }
}
pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let (hover_x, hover_y, drag_x, drag_y) = match status {
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: x,
            is_vertical_scrollbar_hovered: y,
            ..
        } => (x, y, false, false),
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged: x,
            is_vertical_scrollbar_dragged: y,
            ..
        } => (false, false, x, y),
        _ => (false, false, false, false),
    };
    let rail = |hovered, dragged| scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: if dragged {
                alpha(PRIMARY, 0.8)
            } else if hovered {
                alpha(MUTED_FOREGROUND, 0.75)
            } else {
                alpha(SUBTLE, 0.45)
            }
            .into(),
            border: Border {
                radius: radius::circle(scrollbar::THUMB_WIDTH).into(),
                ..Default::default()
            },
        },
    };
    scrollable::Style {
        vertical_rail: rail(hover_y, drag_y),
        horizontal_rail: rail(hover_x, drag_x),
        gap: None,
        ..scrollable::default(theme, status)
    }
}
pub fn select(_: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: FOREGROUND,
        placeholder_color: MUTED_FOREGROUND,
        handle_color: MUTED_FOREGROUND,
        background: INPUT.into(),
        border: border(
            if matches!(status, pick_list::Status::Opened { .. }) {
                RING
            } else {
                BORDER
            },
            radius::CONTROL,
        ),
    }
}
