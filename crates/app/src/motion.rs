//! Motion belongs to the interface design, never to a saved cutting project.
//! All transitions are chosen here so interactions share a consistent feel.

use deepnest_motion::Settings;
use std::sync::OnceLock;

/// Layout changes retain momentum when a better nesting result arrives.
pub fn layout() -> Settings {
    Settings {
        duration: 0.20,
        bounce: 0.25,
        reduced_motion: reduced_motion(),
        ..Settings::default()
    }
}

/// Camera movement settles without overshoot to keep geometry easy to inspect.
pub fn camera() -> Settings {
    Settings {
        bounce: 0.0,
        ..layout()
    }
}

/// Worker updates settle smoothly without bouncing past reported progress.
pub fn progress() -> Settings {
    Settings {
        duration: 0.28,
        bounce: 0.0,
        ..layout()
    }
}

pub fn panel() -> Settings {
    Settings {
        duration: 0.20,
        bounce: 0.0,
        ..layout()
    }
}
pub fn tooltip() -> Settings {
    Settings {
        mode: deepnest_motion::Mode::Easing,
        easing_duration: 0.16,
        bezier: [0.22, 1.0, 0.36, 1.0],
        ..layout()
    }
}

pub const TOOLTIP_DELAY: f64 = 0.35;
pub const TOOLTIP_BRIDGE: f64 = 0.08;
pub const TOOLTIP_WARM: f64 = 0.30;

fn reduced_motion() -> bool {
    static REDUCED: OnceLock<bool> = OnceLock::new();
    *REDUCED.get_or_init(|| {
        // Read the system accessibility preference once at launch. There is no
        // in-app animation editor or project-level animation configuration.
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("/usr/bin/defaults")
                .args(["read", "com.apple.universalaccess", "reduceMotion"])
                .output()
                .is_ok_and(|output| output.status.success() && output.stdout.starts_with(b"1"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepnest_motion::Value;

    #[test]
    fn worker_updates_keep_progress_position_and_velocity_continuous() {
        let settings = Settings {
            reduced_motion: false,
            ..progress()
        };
        let mut value = Value::new(0.);
        value.retarget(0.4, 0., settings);
        value.tick(0.05);
        let (position, velocity) = (value.current, value.velocity);
        assert!(position > 0. && position < 0.4);
        value.retarget(0.8, 0.05, settings);
        assert_eq!(value.current, position);
        assert_eq!(value.velocity, velocity);

        let mut previous = position;
        for frame in 1..=120 {
            value.tick(0.05 + frame as f64 / 60.);
            assert!(value.current >= previous);
            assert!(value.current <= 0.8);
            previous = value.current;
        }
        assert_eq!(value.current, 0.8);
        assert!(!value.is_active());
    }
}
