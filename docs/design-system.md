# Native design system

The UI uses semantic tokens inspired by [shadcn/ui](https://ui.shadcn.com/docs/theming).
The app stays entirely native Iced; React, Tailwind, and CSS are not dependencies.

## One place to change the visual language

Edit `crates/app/src/tokens.rs`. Views consume roles rather than literal colors.

| Token family | Purpose |
| --- | --- |
| `BACKGROUND` / `FOREGROUND` | Application shell and default text |
| `CANVAS` | Continuous drawing-workspace background |
| `SIDEBAR`, `POPOVER`, `MUTED` | Chrome, floating controls, quiet surfaces |
| `PRIMARY` / `PRIMARY_FOREGROUND` | Main action and its readable text/icons |
| `ACCENT` / `ACCENT_FOREGROUND` | Selection and interaction states |
| `MUTED_FOREGROUND`, `SUBTLE` | Supporting labels and tertiary metadata |
| `BORDER`, `INPUT`, `RING` | Separators, fields, focus outlines |
| `DESTRUCTIVE`, `SUCCESS`, `WARNING` | Feedback roles |
| `SHEET` / `SHEET_FOREGROUND` | Dark material surface and light drawing labels |
| `PARTS`, `CUT_LINE`, `SELECTED_PART` | Geometry and selection visualization |
| `radius`, `space`, `typography`, `layout` | Shared geometry, fonts, and density |

Surface colors and foreground colors are paired deliberately: primary buttons
use dark text on electric teal, while the obsidian chrome uses light text. Changing a
surface should include checking its paired foreground.

The workspace uses three inset surfaces: searchable job assets with Parts/Stock
tabs, a flexible canvas with a sheet preview strip, and a 296 px optimizer.
The optimizer keeps export controls in a pinned footer while Setup and Run insights
scroll independently. Search presets configure actual evaluation budgets. Run
insights draw only observed utilization and worker measurements; there are no
synthetic charts. Its circular nesting action uses animated worker progress; the
utilization card shows only verified placement data. Stock controls also appear
in the optimizer when the asset panel is hidden. The canvas uses translucent part fills, distinct
cut outlines, and teal registration corners on dark material. On Windows, matching
window controls replace the system title bar. `windowing.rs` adapts ImagePro's
top-level Win32 subclass: restored windows retain their native resize frame,
maximize uses `HTMAXBUTTON` for Windows Snap Layout hover, and all eight resize
directions use DPI-scaled system metrics. Caption-button geometry comes from the
same shared tokens as the native hit test: a 48 px header and 46 px caption
controls. The header supports dragging,
double-click maximization, and the native context menu. Controls stay accessible
while project menus or settings are open.

The minimum width is 500 logical pixels so Windows can offer side-by-side Snap
layouts. Job assets stays visible at desktop widths, without a hide toggle.
It collapses automatically below 1000 px; the inspector collapses below
800 px, with the nesting action available in the compact header. Native title
bars are retained on other platforms.

Commands have one visible home. Job assets → Parts owns import; the header shows
an import fallback only while that panel is hidden. The project menu owns New,
Open, Save, and Load sample, with keyboard shortcuts beside file actions. The rail
contains Undo and the sole Project settings entry.
Canvas help lives beside the grid control. Empty states explain the workflow
without adding another copy of its commands.

Stock editing lives in Job assets → Stock and moves into Optimizer → Setup only
when the asset panel is hidden. Avoid repeating stock values in summary cards
beside their inputs. Aggregate placement results belong to the optimizer and
appear on the canvas only below the inspector breakpoint. Sheet navigation lives
in the preview strip, with a direct sheet picker for jobs larger than three sheets.
The canvas header identifies the current sheet without repeating its controls.

Corner radii have explicit roles: `CONTROL` is 8 px, `CARD` and `POPOVER` are
12 px, and `PANEL` is 18 px. `TOOL_GROUP` is derived from `CONTROL` plus the
shared 4 px tool inset, so selected buttons and their enclosing tools follow
concentric curves. Use `widgets::tool_group` for grouped canvas controls.

Use `widgets::panel` for workspace panels. An Iced container's rectangular clip
does not clip children to its rounded border. The shared panel finishes child
paint with antialiased corner masks and draws its border last; it also excludes
those corners from pointer hits. This handles full-size canvases, panned geometry,
and scrollbars consistently. Panels sit on `BACKGROUND`. The physical stock
outline remains rectangular, with a flat fill and no offset shadow.

Use `widgets::scroll_region` for vertical scrolling in panels and settings.
Shared `scrollbar` tokens define a 3 px rounded thumb in an 8 px grab area,
a 4 px content gap, and fixed insets around the viewport. The track stays inside
the surface at both scroll limits and reserves space beside the content. Its
thumb becomes brighter on hover and teal while dragging; the rail stays clear.

`style.rs` translates these tokens into reusable Iced styles. Buttons use
`Primary`, `Outline`, `Ghost`, `Selected`, or `Danger`; view code should not create
one-off hover or disabled colors. The line icons are drawn by Iced so missing
system icon fonts cannot change the interface.

`settings.rs` contains nesting and import/export configuration, validation, and
editable drafts. `settings_view.rs` groups those controls into five sections and
draws contextual examples using native Canvas geometry. Explanations change on
hover, editing, or the help button; they remain readable alongside the controls.
These product settings are separate from visual tokens and code-defined motion.

## Language and text

The project menu owns the device-level language selector. Use the catalogs in
`crates/app/locales` and `i18n.rs` for all interface copy, including tooltips,
dropdown labels, drawing annotations, and status messages. Preserve user names
and serialized settings. Iced's automatic shaping provides system font fallback
for Chinese; tooltip measurements reserve a full em for non-ASCII glyphs. Check
both languages when sizing controls or wrapping guidance. See
[localization](localization.md) for translation and validation conventions.

## Motion is a design decision

Edit `crates/app/src/motion.rs`, not the project model. Layout changes use the
shared 0.20-second / 0.25-bounce Time spring. Camera zoom and disclosure panels use
zero bounce. `widgets.rs` clips folded content instead of shrinking its text and
controls. Nesting progress uses a 0.28-second zero-bounce spring for both the
3 px rounded progress ring. Worker data stays immediate;
the display keeps requesting frames through completion and reset until it settles.
`tooltip.rs` owns one moving hint surface, with a 350 ms initial delay,
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
