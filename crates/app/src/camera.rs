//! The same transform drives rendering, hit testing, and cursor-anchored zoom.
use crate::model::Stock;
use iced::{Point, Size, Vector};
use nest::Pt;

pub fn transform(stock: &Stock, viewport: Size, zoom: f64, pan: Vector) -> (f32, Point) {
    let scale = ((viewport.width - 144.0).max(20.0) / stock.width as f32)
        .min((viewport.height - 192.0).max(20.0) / stock.height as f32)
        * zoom as f32;
    let origin = Point::new(
        (viewport.width - stock.width as f32 * scale) / 2.0,
        (viewport.height - stock.height as f32 * scale) / 2.0,
    ) + pan;
    (scale, origin)
}

#[derive(Clone, Copy)]
pub struct Anchor {
    screen: Point,
    world: Pt,
    viewport: Size,
}

impl Anchor {
    pub fn new(stock: &Stock, viewport: Size, zoom: f64, pan: Vector, screen: Point) -> Self {
        let (scale, origin) = transform(stock, viewport, zoom, pan);
        Self {
            screen,
            world: Pt::new(
                ((screen.x - origin.x) / scale) as f64,
                ((screen.y - origin.y) / scale) as f64,
            ),
            viewport,
        }
    }

    /// Apply this on every animation frame, not just when a wheel event arrives.
    pub fn pan(self, stock: &Stock, zoom: f64) -> Vector {
        let (scale, origin) = transform(stock, self.viewport, zoom, Vector::ZERO);
        self.screen - origin - Vector::new(self.world.x as f32 * scale, self.world.y as f32 * scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use deepnest_motion::{Settings, Value};

    fn assert_anchored(anchor: Anchor, stock: &Stock, zoom: f64) {
        let (scale, origin) = transform(stock, anchor.viewport, zoom, anchor.pan(stock, zoom));
        let projected =
            origin + Vector::new(anchor.world.x as f32 * scale, anchor.world.y as f32 * scale);
        assert!((projected.x - anchor.screen.x).abs() < 0.001);
        assert!((projected.y - anchor.screen.y).abs() < 0.001);
    }

    #[test]
    fn wheel_anchor_stays_fixed_through_spring_frames_and_reversal() {
        let stock = Project::demo().stock;
        let viewport = Size::new(1100., 760.);
        let anchor = Anchor::new(
            &stock,
            viewport,
            1.3,
            Vector::new(87., -42.),
            Point::new(210., 570.),
        );
        let settings = Settings {
            bounce: 0.,
            ..Settings::default()
        };
        let mut zoom = Value::new(1.3);
        zoom.retarget(2.4, 0., settings);
        for frame in 0..120 {
            let now = frame as f64 / 120.;
            if frame == 10 {
                zoom.retarget(0.6, now, settings);
            }
            zoom.tick(now);
            assert_anchored(anchor, &stock, zoom.current);
        }
        assert!(!zoom.is_active());
        assert_eq!(zoom.current, 0.6);
    }

    #[test]
    fn moving_the_cursor_reanchors_the_displayed_sheet_without_a_jump() {
        let stock = Project::demo().stock;
        let viewport = Size::new(1000., 680.);
        let old = Anchor::new(&stock, viewport, 1., Vector::ZERO, Point::new(200., 400.));
        let pan = old.pan(&stock, 1.7);
        let next = Anchor::new(&stock, viewport, 1.7, pan, Point::new(740., 160.));
        let unchanged = next.pan(&stock, 1.7);
        assert!((unchanged.x - pan.x).abs() < 0.001);
        assert!((unchanged.y - pan.y).abs() < 0.001);
        for zoom in [0.25, 0.5, 1., 3., 5.] {
            assert_anchored(next, &stock, zoom);
        }
    }
}
