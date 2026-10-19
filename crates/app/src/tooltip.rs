//! One moving tooltip surface for the whole workspace, with shared warm-up.
use crate::{
    motion,
    tokens::{color::*, radius, typography::*},
    Message,
};
use deepnest_motion::Value;
use iced::{
    mouse,
    widget::canvas::{self, Frame, Geometry, Path},
    Point, Rectangle, Renderer, Size, Theme, Vector,
};

pub struct Tooltip {
    pub text: &'static str,
    x: Value,
    y: Value,
    width: Value,
    opacity: Value,
    hovered: Option<&'static str>,
    pending: Option<(&'static str, Rectangle, Size, f64)>,
    hide_at: Option<f64>,
    last_exit: f64,
}
impl Default for Tooltip {
    fn default() -> Self {
        Self {
            text: "",
            x: Value::new(0.),
            y: Value::new(0.),
            width: Value::new(0.),
            opacity: Value::new(0.),
            hovered: None,
            pending: None,
            hide_at: None,
            last_exit: -1.,
        }
    }
}
impl Tooltip {
    pub fn enter(&mut self, label: &'static str, bounds: Rectangle, window: Size, now: f64) {
        self.hovered = Some(label);
        self.hide_at = None;
        if self.opacity.current > 0.01 || now - self.last_exit < motion::TOOLTIP_WARM {
            self.show(label, bounds, window, now);
        } else {
            self.pending = Some((label, bounds, window, now + motion::TOOLTIP_DELAY));
        }
    }
    pub fn leave(&mut self, label: &'static str, now: f64) {
        if self.hovered == Some(label) {
            self.hovered = None;
            self.pending = None;
            if self.opacity.target > 0. {
                self.last_exit = now;
                self.hide_at = Some(now + motion::TOOLTIP_BRIDGE);
            }
        }
    }
    pub fn dismiss(&mut self, now: f64) {
        self.hovered = None;
        self.pending = None;
        self.hide_at = None;
        self.last_exit = -1.;
        self.opacity.retarget(0., now, motion::tooltip());
    }
    fn show(&mut self, label: &'static str, bounds: Rectangle, window: Size, now: f64) {
        // CJK glyphs occupy a full em; ASCII hints retain the compact mono spacing.
        let width = label
            .chars()
            .map(|c| if c.is_ascii() { 6.7 } else { SMALL as f64 })
            .sum::<f64>()
            + 24.;
        let x = (bounds.center_x() as f64 - width / 2.)
            .clamp(8., (window.width as f64 - width - 8.).max(8.));
        let y = if bounds.y + bounds.height + 44. < window.height {
            bounds.y + bounds.height + 8.
        } else {
            bounds.y - 40.
        };
        self.text = label;
        self.pending = None;
        if self.opacity.current < 0.01 {
            self.x.jump(x);
            self.y.jump(y as f64);
            self.width.jump(width);
        } else {
            self.x.retarget(x, now, motion::tooltip());
            self.y.retarget(y as f64, now, motion::tooltip());
            self.width.retarget(width, now, motion::tooltip());
        }
        self.opacity.retarget(1., now, motion::tooltip());
    }
    pub fn tick(&mut self, now: f64) {
        if let Some((label, bounds, window, at)) = self.pending {
            if now >= at {
                self.show(label, bounds, window, now);
            }
        }
        if self.hide_at.is_some_and(|at| now >= at) {
            self.hide_at = None;
            self.opacity.retarget(0., now, motion::tooltip());
        }
        self.x.tick(now);
        self.y.tick(now);
        self.width.tick(now);
        self.opacity.tick(now);
    }
    pub fn active(&self) -> bool {
        self.pending.is_some()
            || self.hide_at.is_some()
            || [&self.x, &self.y, &self.width, &self.opacity]
                .iter()
                .any(|v| v.is_active())
    }
    pub fn visible(&self) -> bool {
        self.opacity.current > 0.001
    }
}

pub struct Surface<'a>(pub &'a Tooltip);
impl canvas::Program<Message> for Surface<'_> {
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
        let t = self.0;
        let opacity = t.opacity.current as f32;
        let position = Point::new(t.x.current as f32, t.y.current as f32 + (1. - opacity) * 3.);
        let size = Size::new(t.width.current as f32, 32.);
        frame.fill(
            &Path::rounded_rectangle(position + Vector::new(0., 3.), size, radius::TOOLTIP.into()),
            SHADOW.scale_alpha(opacity),
        );
        frame.fill(
            &Path::rounded_rectangle(position, size, radius::TOOLTIP.into()),
            POPOVER.scale_alpha(opacity),
        );
        frame.stroke(
            &Path::rounded_rectangle(position, size, radius::TOOLTIP.into()),
            canvas::Stroke::default()
                .with_width(1.)
                .with_color(BORDER.scale_alpha(opacity)),
        );
        frame.fill_text(canvas::Text {
            content: t.text.into(),
            position: position + Vector::new(12., 16.),
            size: SMALL.into(),
            font: MONO,
            color: FOREGROUND.scale_alpha(opacity),
            align_y: iced::alignment::Vertical::Center,
            ..Default::default()
        });
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neighboring_controls_share_delay_and_retarget_without_closing() {
        let mut t = Tooltip::default();
        let bounds = Rectangle::new(Point::new(100., 100.), Size::new(32., 32.));
        let window = Size::new(1000., 800.);
        t.enter("First", bounds, window, 0.);
        t.tick(0.2);
        assert_eq!(t.opacity.target, 0.);
        t.tick(0.36);
        t.tick(0.60);
        assert_eq!(t.opacity.target, 1.);
        let x = t.x.current;
        t.leave("First", 0.61);
        t.enter("Second", Rectangle { x: 140., ..bounds }, window, 0.63);
        assert_eq!(t.text, "Second");
        assert_eq!(t.opacity.target, 1.);
        assert!(t.pending.is_none());
        if !motion::tooltip().reduced_motion {
            assert_eq!(t.x.current, x);
        }
        assert!(t.x.target > x);
        t.leave("First", 0.64);
        assert_eq!(t.hovered, Some("Second"));
        t.leave("Second", 0.7);
        t.tick(0.9);
        t.tick(1.2);
        assert!(!t.visible());
        assert!(!t.active());
    }
    #[test]
    fn leaving_during_initial_delay_never_flashes_a_tooltip() {
        let mut t = Tooltip::default();
        t.enter("Test", Rectangle::default(), Size::new(1000., 800.), 0.);
        t.leave("Test", 0.1);
        t.tick(1.);
        assert!(!t.visible());
        assert!(!t.active());
    }
}
