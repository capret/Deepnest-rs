//! The single source of truth for the Graphite theme.
//!
//! Semantic surface/foreground pairs follow shadcn's token convention. Views
//! choose roles; they never choose raw colors. Component state styles live in
//! `style.rs`, and interaction timing lives in `motion.rs`.

pub mod color {
    use iced::Color;
    pub const BACKGROUND: Color = Color::from_rgb8(27, 29, 31);
    pub const FOREGROUND: Color = Color::from_rgb8(232, 235, 237);
    pub const SIDEBAR: Color = Color::from_rgb8(35, 37, 40);
    pub const POPOVER: Color = Color::from_rgb8(43, 46, 49);
    pub const MUTED: Color = Color::from_rgb8(45, 48, 52);
    pub const MUTED_FOREGROUND: Color = Color::from_rgb8(157, 164, 172);
    pub const SUBTLE: Color = Color::from_rgb8(112, 120, 128);
    pub const PRIMARY: Color = Color::from_rgb8(190, 227, 204);
    pub const PRIMARY_FOREGROUND: Color = Color::from_rgb8(25, 53, 37);
    pub const ACCENT: Color = Color::from_rgb8(48, 65, 58);
    pub const ACCENT_FOREGROUND: Color = Color::from_rgb8(205, 232, 216);
    pub const BORDER: Color = Color::from_rgb8(55, 59, 63);
    pub const INPUT: Color = Color::from_rgb8(29, 32, 35);
    pub const RING: Color = PRIMARY;
    pub const DESTRUCTIVE: Color = Color::from_rgb8(239, 155, 148);
    pub const SUCCESS: Color = Color::from_rgb8(147, 199, 168);
    pub const WARNING: Color = Color::from_rgb8(225, 192, 133);
    // Drawing surfaces pair dark ink with light material.
    pub const SHEET: Color = Color::from_rgb8(241, 242, 236);
    pub const SHEET_FOREGROUND: Color = Color::from_rgb8(65, 78, 75);
    pub const SHEET_BORDER: Color = Color::from_rgb8(201, 207, 198);
    pub const CUT_LINE: Color = Color::from_rgb8(115, 133, 127);
    pub const SELECTED_PART: Color = Color::from_rgb8(188, 215, 198);
    pub const SELECTED_CUT_LINE: Color = Color::from_rgb8(62, 111, 86);
    pub const GRID: Color = Color::from_rgb8(46, 50, 53);
    pub const SHADOW: Color = Color::from_rgba8(0, 0, 0, 0.25);
    pub const PARTS: [Color; 5] = [
        Color::from_rgb8(198, 209, 204),
        Color::from_rgb8(195, 207, 216),
        Color::from_rgb8(219, 210, 191),
        Color::from_rgb8(210, 203, 218),
        Color::from_rgb8(193, 214, 213),
    ];
}

pub mod radius {
    pub const BASE: f32 = 8.0;
    pub const SM: f32 = BASE * 0.5;
    pub const MD: f32 = BASE * 0.75;
    pub const XL: f32 = BASE * 1.5;
}
pub mod space {
    pub const XXS: f32 = 4.0;
    pub const XS: f32 = 8.0;
    pub const SM: f32 = 12.0;
    pub const MD: f32 = 16.0;
    pub const LG: f32 = 24.0;
    pub const XL: f32 = 32.0;
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
    pub const TOOLBAR: f32 = 56.0;
    pub const SIDEBAR: f32 = 260.0;
    pub const STATUS: f32 = 28.0;
    pub const CONTROL: f32 = 32.0;
    pub const ICON: f32 = 16.0;
}
