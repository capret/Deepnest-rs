//! One motion vocabulary for the entire interface. Time springs use visual
//! duration; physics springs use physical units; easing uses cubic Bezier.
//! Sampling is analytic, independent of frame rate, and spring retargets carry
//! their current velocity. This crate has no UI/runtime dependency.

use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Time,
    Physics,
    Easing,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Time => "Time",
            Self::Physics => "Physics",
            Self::Easing => "Easing",
        })
    }
}

/// Code-defined transition parameters. The same value can be shared by every
/// animated property in an interaction; these are not product preferences.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub duration: f64,
    pub bounce: f64,
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
    pub easing_duration: f64,
    pub bezier: [f64; 4],
    pub reduced_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Time,
            duration: 0.20,
            bounce: 0.25,
            stiffness: 320.0,
            damping: 26.0,
            mass: 1.0,
            easing_duration: 0.20,
            bezier: [0.22, 1.0, 0.36, 1.0],
            reduced_motion: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        let valid =
            |value: f64, min: f64, max: f64| value.is_finite() && (min..=max).contains(&value);
        if !valid(self.duration, 0.05, 2.0)
            || !valid(self.bounce, 0.0, 0.8)
            || !valid(self.stiffness, 20.0, 1000.0)
            || !valid(self.damping, 1.0, 100.0)
            || !valid(self.mass, 0.1, 5.0)
            || !valid(self.easing_duration, 0.05, 2.0)
            || !valid(self.bezier[0], 0.0, 1.0)
            || !valid(self.bezier[2], 0.0, 1.0)
            || !valid(self.bezier[1], -1.0, 2.0)
            || !valid(self.bezier[3], -1.0, 2.0)
        {
            return Err("Motion parameters are outside the supported ranges.");
        }
        Ok(())
    }

    pub fn transition(self) -> Transition {
        match self.mode {
            Mode::Time => {
                // Designer-facing visual duration: the main movement finishes
                // around this time, with any residual bounce settling afterward.
                let omega = TAU / (self.duration.max(0.01) * 1.2);
                let ratio = (1.0 - self.bounce).clamp(0.05, 1.0);
                Transition::Spring {
                    stiffness: omega * omega,
                    damping: 2.0 * ratio * omega,
                    mass: 1.0,
                }
            }
            Mode::Physics => Transition::Spring {
                stiffness: self.stiffness.max(0.01),
                damping: self.damping.max(0.01),
                mass: self.mass.max(0.01),
            },
            Mode::Easing => Transition::Easing {
                duration: self.easing_duration.max(0.01),
                bezier: self.bezier,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Transition {
    Spring {
        stiffness: f64,
        damping: f64,
        mass: f64,
    },
    Easing {
        duration: f64,
        bezier: [f64; 4],
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub value: f64,
    pub velocity: f64,
}

impl Transition {
    pub fn sample(self, time: f64, from: f64, to: f64, velocity: f64) -> Sample {
        let time = time.max(0.0);
        match self {
            Self::Spring {
                stiffness,
                damping,
                mass,
            } => {
                let omega = (stiffness / mass).sqrt();
                let decay = damping / (2.0 * mass);
                let displacement = from - to;
                let (x, v) = if (decay - omega).abs() < 1e-6 {
                    let b = velocity + omega * displacement;
                    let e = (-omega * time).exp();
                    let x = (displacement + b * time) * e;
                    (x, b * e - omega * x)
                } else if decay < omega {
                    let frequency = (omega * omega - decay * decay).sqrt();
                    let b = (velocity + decay * displacement) / frequency;
                    let (sin, cos) = (frequency * time).sin_cos();
                    let e = (-decay * time).exp();
                    let x = e * (displacement * cos + b * sin);
                    let v = e * frequency * (b * cos - displacement * sin) - decay * x;
                    (x, v)
                } else {
                    let root = (decay * decay - omega * omega).sqrt();
                    let r1 = -decay + root;
                    let r2 = -decay - root;
                    let a = (velocity - r2 * displacement) / (r1 - r2);
                    let b = displacement - a;
                    let e1 = a * (r1 * time).exp();
                    let e2 = b * (r2 * time).exp();
                    (e1 + e2, r1 * e1 + r2 * e2)
                };
                Sample {
                    value: to + x,
                    velocity: v,
                }
            }
            Self::Easing { duration, bezier } => {
                let progress = (time / duration).clamp(0.0, 1.0);
                let value = cubic_bezier(progress, bezier);
                let epsilon = 0.0001;
                let lower = (progress - epsilon).max(0.0);
                let upper = (progress + epsilon).min(1.0);
                let slope = if progress >= 1.0 {
                    0.0
                } else {
                    (cubic_bezier(upper, bezier) - cubic_bezier(lower, bezier))
                        / (upper - lower)
                        / duration
                };
                Sample {
                    value: from + (to - from) * value,
                    velocity: (to - from) * slope,
                }
            }
        }
    }
}

/// Invert x(t) before sampling y(t). Sampling y(progress) directly is incorrect
/// for asymmetric control points, and is a common easing implementation bug.
pub fn cubic_bezier(progress: f64, [x1, y1, x2, y2]: [f64; 4]) -> f64 {
    if progress <= 0.0 {
        return 0.0;
    }
    if progress >= 1.0 {
        return 1.0;
    }
    let curve = |t: f64, a: f64, b: f64| {
        3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t * t * b + t.powi(3)
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..32 {
        let mid = (low + high) * 0.5;
        if curve(mid, x1, x2) < progress {
            low = mid;
        } else {
            high = mid;
        }
    }
    curve((low + high) * 0.5, y1, y2)
}

#[derive(Debug, Clone)]
pub struct Value {
    pub current: f64,
    pub velocity: f64,
    pub target: f64,
    from: f64,
    initial_velocity: f64,
    started: f64,
    transition: Transition,
    active: bool,
}

impl Value {
    pub fn new(value: f64) -> Self {
        Self {
            current: value,
            target: value,
            from: value,
            velocity: 0.0,
            initial_velocity: 0.0,
            started: 0.0,
            transition: Settings::default().transition(),
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn jump(&mut self, target: f64) {
        self.current = target;
        self.target = target;
        self.velocity = 0.0;
        self.active = false;
    }

    pub fn retarget(&mut self, target: f64, now: f64, settings: Settings) {
        self.tick(now);
        if settings.reduced_motion {
            self.jump(target);
            return;
        }
        self.from = self.current;
        self.initial_velocity = self.velocity;
        self.target = target;
        self.started = now;
        self.transition = settings.transition();
        self.active = (self.target - self.current).abs() > 1e-8 || self.velocity.abs() > 1e-8;
    }

    pub fn tick(&mut self, now: f64) {
        if !self.active {
            return;
        }
        let sample = self.transition.sample(
            now - self.started,
            self.from,
            self.target,
            self.initial_velocity,
        );
        self.current = sample.value;
        self.velocity = sample.velocity;
        let tolerance = (self.target - self.from).abs().max(1.0) * 0.0001;
        if (self.current - self.target).abs() < tolerance && self.velocity.abs() < tolerance {
            self.jump(self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_requested_transition() {
        let s = Settings::default();
        assert_eq!((s.mode, s.bounce, s.duration), (Mode::Time, 0.25, 0.20));
        assert!(s.validate().is_ok());
    }

    #[test]
    fn springs_have_correct_initial_conditions_and_settle_in_all_regimes() {
        for damping in [6.0, 20.0, 40.0] {
            let s = Transition::Spring {
                stiffness: 100.0,
                damping,
                mass: 1.0,
            };
            let start = s.sample(0.0, 7.0, 12.0, -3.0);
            assert!((start.value - 7.0).abs() < 1e-10);
            assert!((start.velocity + 3.0).abs() < 1e-10);
            let end = s.sample(15.0, 7.0, 12.0, -3.0);
            assert!((end.value - 12.0).abs() < 1e-6);
            assert!(end.velocity.abs() < 1e-6);
        }
    }

    #[test]
    fn time_spring_has_subtle_overshoot_and_reaches_target_around_visual_duration() {
        let s = Settings::default();
        assert!((s.transition().sample(0.20, 0.0, 1.0, 0.0).value - 1.0).abs() < 0.05);
        assert!(
            (1..100).any(|i| s.transition().sample(i as f64 / 100.0, 0.0, 1.0, 0.0).value > 1.01)
        );
        assert!((s.transition().sample(1.0, 0.0, 1.0, 0.0).value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_bounce_is_monotonic() {
        let s = Settings {
            bounce: 0.0,
            ..Settings::default()
        }
        .transition();
        let mut previous = 0.0;
        for i in 0..100 {
            let value = s.sample(i as f64 / 100.0, 0.0, 1.0, 0.0).value;
            assert!(value >= previous && value <= 1.0);
            previous = value;
        }
    }

    #[test]
    fn reversal_preserves_position_and_velocity() {
        let settings = Settings::default();
        let mut value = Value::new(0.0);
        value.retarget(100.0, 0.0, settings);
        value.tick(0.05);
        let before = (value.current, value.velocity);
        value.retarget(0.0, 0.05, settings);
        assert_eq!(before, (value.current, value.velocity));
        value.tick(0.050001);
        assert!((before.0 - value.current).abs() < 0.01);
        assert!((before.1 - value.velocity).abs() < 0.1);
        value.tick(2.0);
        assert_eq!(value.current, 0.0);
        assert!(!value.is_active());
    }

    #[test]
    fn sampling_is_frame_rate_independent() {
        let mut a = Value::new(0.0);
        a.retarget(1.0, 0.0, Settings::default());
        let mut b = a.clone();
        for i in 1..10 {
            a.tick(i as f64 * 0.01);
        }
        a.tick(0.1);
        b.tick(0.1);
        assert_eq!(a.current, b.current);
    }

    #[test]
    fn reduced_motion_snaps_and_stops_in_flight_animations() {
        let mut value = Value::new(0.0);
        value.retarget(1.0, 0.0, Settings::default());
        value.retarget(
            4.0,
            0.05,
            Settings {
                reduced_motion: true,
                ..Settings::default()
            },
        );
        assert_eq!(value.current, 4.0);
        assert!(!value.is_active());
    }

    #[test]
    fn bezier_inverts_time_axis_and_supports_overshoot() {
        for i in 0..100 {
            let t = i as f64 / 100.0;
            assert!((cubic_bezier(t, [0.3, 0.3, 0.7, 0.7]) - t).abs() < 1e-8);
        }
        assert!((cubic_bezier(0.5, [0.0, 0.0, 0.0, 1.0]) - 0.88988157).abs() < 1e-6);
        assert!(cubic_bezier(0.8, [0.3, 1.5, 0.6, 1.5]) > 1.0);
    }

    #[test]
    fn invalid_imported_parameters_are_rejected() {
        assert!(Settings {
            mass: 0.0,
            ..Settings::default()
        }
        .validate()
        .is_err());
        assert!(Settings {
            duration: f64::NAN,
            ..Settings::default()
        }
        .validate()
        .is_err());
        assert!(Settings {
            bezier: [-0.1, 0.0, 1.0, 1.0],
            ..Settings::default()
        }
        .validate()
        .is_err());
    }
}
