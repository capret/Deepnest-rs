# Native design system

The UI uses semantic tokens inspired by [shadcn/ui](https://ui.shadcn.com/docs/theming).
The app stays entirely native Iced; React, Tailwind, and CSS are not dependencies.

## One place to change the visual language

Edit `crates/app/src/tokens.rs`. Views consume roles rather than literal colors.

| Token family | Purpose |
| --- | --- |
| `BACKGROUND` / `FOREGROUND` | Main canvas and default text |
| `SIDEBAR`, `POPOVER`, `MUTED` | Chrome, floating controls, quiet surfaces |
| `PRIMARY` / `PRIMARY_FOREGROUND` | Main action and its readable text/icons |
| `ACCENT` / `ACCENT_FOREGROUND` | Selection and interaction states |
| `MUTED_FOREGROUND`, `SUBTLE` | Supporting labels and tertiary metadata |
| `BORDER`, `INPUT`, `RING` | Separators, fields, focus outlines |
| `DESTRUCTIVE`, `SUCCESS`, `WARNING` | Feedback roles |
| `SHEET` / `SHEET_FOREGROUND` | Light physical material and dark drawing labels |
| `PARTS`, `CUT_LINE`, `SELECTED_PART` | Geometry and selection visualization |
| `radius`, `space`, `typography`, `layout` | Shared geometry, fonts, and density |

Surface colors and foreground colors are paired deliberately: primary buttons
use dark text on mint, while the graphite chrome uses light text. Changing a
surface should include checking its paired foreground.

`style.rs` translates these tokens into reusable Iced styles. Buttons use
`Primary`, `Outline`, `Ghost`, `Selected`, or `Danger`; view code should not create
one-off hover or disabled colors. The line icons are drawn by Iced so missing
system icon fonts cannot change the interface.

`settings.rs` contains nesting and import/export configuration, validation, and
editable drafts. `settings_view.rs` groups those controls into five sections and
draws contextual examples using native Canvas geometry. Explanations change on
hover, editing, or the help button; they remain readable alongside the controls.
These product settings are separate from visual tokens and code-defined motion.

## Motion is a design decision

Edit `crates/app/src/motion.rs`, not the project model. Layout changes use the
shared 0.20-second / 0.25-bounce Time spring. Camera zoom and disclosure panels use
zero bounce. `widgets.rs` clips folded content instead of shrinking its text and
controls. `tooltip.rs` owns one moving hint surface, with a 350 ms initial delay,
an 80 ms bridge across gaps, and a 160 ms Bézier transition between targets.
Panning and input edits remain immediate. The app stops requesting animation
frames once all animated values settle.

The pure Rust `deepnest-motion` crate also supports physical and easing
transitions. For example, a designer can change a token in code:

```rust
use deepnest_motion::{Mode, Settings};

let time = Settings {
    mode: Mode::Time,
    duration: 0.20,
    bounce: 0.25,
    ..Settings::default()
};
let physics = Settings {
    mode: Mode::Physics,
    stiffness: 320.0,
    damping: 26.0,
    mass: 1.0,
    ..time
};
let easing = Settings {
    mode: Mode::Easing,
    easing_duration: 0.20,
    bezier: [0.22, 1.0, 0.36, 1.0],
    ..time
};
```

Retarget an existing `Value` to preserve its current spring velocity. Use `jump`
for immediate changes. Time duration describes the main movement; a spring's
residual motion settles afterward. Bézier timing inverts the x-axis before
sampling the y-axis. These are native implementations of the same parameter
families exposed by [DialKit](https://github.com/joshpuckett/dialkit), not a claim
of bit-for-bit parity with Motion's spring solver.

Motion settings are never serialized in cutting projects. Earlier prototype
project files containing a `motion` key still open, and that key is ignored.
The macOS Reduce Motion preference is respected automatically when the first
transition is initialized; a preference change takes effect after relaunch.
