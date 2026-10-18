//! The single source of truth for the Obsidian workspace theme.
//!
//! Semantic surface/foreground pairs follow shadcn's token convention. Views
//! choose roles; they never choose raw colors. Component state styles live in
//! `style.rs`, and interaction timing lives in `motion.rs`.

pub mod color {
    use iced::Color;
    pub const BACKGROUND: Color = Color::from_rgb8(12, 15, 16);
    pub const CANVAS: Color = Color::from_rgb8(15, 21, 22);
    pub const FOREGROUND: Color = Color::from_rgb8(235, 242, 240);
    pub const SIDEBAR: Color = Color::from_rgb8(22, 26, 27);
    pub const POPOVER: Color = Color::from_rgb8(29, 34, 35);
    pub const MUTED: Color = Color::from_rgb8(34, 40, 41);
    pub const MUTED_FOREGROUND: Color = Color::from_rgb8(153, 168, 166);
    pub const SUBTLE: Color = Color::from_rgb8(117, 136, 133);
    pub const PRIMARY: Color = Color::from_rgb8(29, 235, 205);
    pub const PRIMARY_FOREGROUND: Color = Color::from_rgb8(5, 45, 40);
    pub const ACCENT: Color = Color::from_rgb8(28, 57, 53);
    pub const ACCENT_FOREGROUND: Color = Color::from_rgb8(196, 255, 243);
    pub const BORDER: Color = Color::from_rgb8(41, 50, 50);
    pub const INPUT: Color = Color::from_rgb8(17, 22, 23);
    pub const RING: Color = PRIMARY;
    pub const DESTRUCTIVE: Color = Color::from_rgb8(239, 155, 148);
    pub const SUCCESS: Color = Color::from_rgb8(71, 211, 170);
    pub const WARNING: Color = Color::from_rgb8(225, 192, 133);
    pub const WINDOW_CLOSE_HOVER: Color = Color::from_rgb8(196, 44, 54);
    pub const WINDOW_CLOSE_PRESSED: Color = Color::from_rgb8(153, 32, 42);
    // Dark material lets cut lines and the selected geometry carry the light.
    pub const SHEET: Color = Color::from_rgb8(18, 27, 28);
    pub const SHEET_FOREGROUND: Color = Color::from_rgb8(200, 226, 219);
    pub const SHEET_BORDER: Color = Color::from_rgb8(59, 91, 85);
    pub const CUT_LINE: Color = Color::from_rgb8(112, 164, 155);
    pub const SELECTED_PART: Color = Color::from_rgb8(30, 92, 81);
    pub const SELECTED_CUT_LINE: Color = PRIMARY;
    pub const GRID: Color = Color::from_rgb8(35, 47, 47);
    pub const SHADOW: Color = Color::from_rgba8(0, 0, 0, 0.40);
    pub const PARTS: [Color; 5] = [
        Color::from_rgb8(79, 193, 164),
        Color::from_rgb8(111, 166, 203),
        Color::from_rgb8(202, 174, 117),
        Color::from_rgb8(165, 145, 199),
        Color::from_rgb8(102, 182, 183),
    ];
}

pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const BADGE: f32 = 4.0;
    pub const CONTROL: f32 = 8.0;
    pub const TOOLTIP: f32 = CONTROL;
    pub const CARD: f32 = 12.0;
    pub const POPOVER: f32 = 12.0;
    pub const PANEL: f32 = 18.0;
    // Nested tool buttons and their enclosing surface follow the same curve.
    pub const TOOL_GROUP: f32 = CONTROL + super::layout::TOOL_GROUP_INSET;

    pub const fn inset(outer: f32, padding: f32) -> f32 {
        if outer > padding {
            outer - padding
        } else {
            0.0
        }
    }

    pub const fn circle(diameter: f32) -> f32 {
        diameter / 2.0
    }
}
pub mod space {
    pub const XXS: f32 = 4.0;
    pub const XS: f32 = 8.0;
    pub const SM: f32 = 12.0;
    pub const MD: f32 = 16.0;
    pub const LG: f32 = 24.0;
}
pub mod scrollbar {
    pub const THUMB_WIDTH: f32 = 3.0;
    pub const RAIL_WIDTH: f32 = 8.0;
    pub const CONTENT_GAP: f32 = 4.0;
    pub const EDGE_INSET: f32 = 4.0;
    pub const END_INSET: f32 = 8.0;
}
pub mod typography {
    use iced::Font;
    pub const CAPTION: f32 = 10.0;
    pub const SMALL: f32 = 11.0;
    pub const BODY: f32 = 12.0;
    pub const HEADING: f32 = 14.0;
    pub const TITLE: f32 = 20.0;
    pub const BOLD: Font = Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    };
    pub const MONO: Font = Font::MONOSPACE;
}
pub mod layout {
    pub const TOOLBAR: f32 = 48.0;
    pub const SIDEBAR: f32 = 248.0;
    pub const INSPECTOR: f32 = 296.0;
    pub const RAIL: f32 = 56.0;
    pub const STATUS: f32 = 28.0;
    pub const CONTROL: f32 = 32.0;
    pub const TOOL_GROUP_INSET: f32 = 4.0;
    pub const ICON: f32 = 16.0;
    pub const WINDOW_CONTROL_WIDTH: f32 = 46.0;
    pub const WINDOW_TITLEBAR_HEIGHT: f32 = TOOLBAR;
    pub const WINDOW_GLYPH: f32 = 14.0;
}
