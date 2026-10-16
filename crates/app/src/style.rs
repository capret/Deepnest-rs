//! Reusable component variants built exclusively from semantic design tokens.
use crate::tokens::{color::*, radius};
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
pub fn surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SIDEBAR.into()),
        text_color: Some(FOREGROUND),
        ..Default::default()
    }
}
pub fn floating(_: &Theme) -> container::Style {
    container::Style {
        background: Some(POPOVER.into()),
        text_color: Some(FOREGROUND),
        border: border(BORDER, radius::XL),
        shadow: Shadow {
            color: SHADOW,
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    }
}
pub fn inset(_: &Theme) -> container::Style {
    container::Style {
        background: Some(INPUT.into()),
        border: border(BORDER, radius::MD),
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
            Button::Selected => (ACCENT, ACCENT_FOREGROUND, Color::TRANSPARENT),
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
            border: border(if disabled { alpha(line, 0.4) } else { line }, radius::MD),
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
            radius::MD,
        ),
        icon: MUTED_FOREGROUND,
        placeholder: SUBTLE,
        value: FOREGROUND,
        selection: ACCENT,
    }
}
pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    style.vertical_rail.background = None;
    style.vertical_rail.scroller.background = BORDER.into();
    style
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
            radius::MD,
        ),
    }
}
