# Deepnest

A native Rust nesting workspace built with **Iced 0.14**. Import SVG or DXF parts,
set quantities and stock dimensions, find a cutting layout, and export SVG or DXF.

The Obsidian interface pairs a dark precision canvas with teal cut lines and
rounded, layered panels. Searchable job assets, separate Parts/Stock tabs, clickable
sheet previews, and an optimizer with Setup/Run insights keep the entire job in one
workspace. Live measurements include elapsed time, valid candidates, time to the
first layout, geometry preparation, validation, and NFP cache reuse. Animation is part of the interface
design: there are no animation controls or animation settings in project files.

## Run

Install a Rust toolchain (1.88 or later), then:

```sh
cargo run -p deepnest-app
cargo test --workspace
cargo build --release -p deepnest-app
```

Iced uses a native window and a GPU renderer. Cargo builds the entire application.
On Linux, the platform's windowing/graphics and desktop file-dialog services must
be available.

On macOS, build an application bundle:

```sh
bash scripts/bundle-macos.sh          # target/release/Deepnest.app
bash scripts/bundle-macos.sh --debug  # target/debug/Deepnest.app
```

These are local development bundles, without signing or notarization.

## Workflow

1. Import local SVG/DXF files, or use the bundled Assembly kit sample. Drag-and-drop
   works for both formats and `.deepnest` projects.
2. Select a part in the sidebar or canvas to change its quantity. The source
   preview shows one of each shape; nesting uses all copies.
3. Open **Stock** in Job assets to set width, height, sheet count, and part gap in your display units. Apply edits
   with Enter or **Apply**; nesting and saving also validate pending edits.
4. Choose a Quick (16), Balanced (64), or Deep (256) evaluation budget, or search
   until stopped, then press **Nest**. **Run insights** shows actual measurements
   and observed material use. **Stop** keeps the best verified result.
5. Switch sheets using the preview strip or sheet selector, pan, zoom, and inspect the result. Choose **SVG** or **DXF** in the optimizer, then
   **Export layout**. Placed sheets are stacked vertically. SVG uses millimeters; DXF
   uses the chosen export units and a separate layer for each sheet. Shared-line
   cutting exports each touching edge once when the part gap is zero.
6. Save a `.deepnest` project to retain source geometry, quantities, stock, and
   search settings. Layouts are recomputed on demand. Undo restores project edits.

The footer reports unplaced parts when stock is insufficient. A partial layout
can be exported; it contains only the placed parts.

| Shortcut | Action |
| --- | --- |
| Cmd/Ctrl + I | Import SVG / DXF |
| Cmd/Ctrl + O | Open project |
| Cmd/Ctrl + S | Save project |
| Cmd/Ctrl + E | Export cutting layout |
| Cmd/Ctrl + Enter | Start nesting |
| Cmd/Ctrl + Z | Undo |
| Cmd/Ctrl + , | Nesting settings and illustrated guide |
| Escape | Stop nesting / close menus |
| Tab / Shift + Tab | Move between editable controls |

Scroll to zoom around the mouse pointer, drag empty canvas (or middle-drag
anywhere) to pan, and use the fit button to recenter. The point under the cursor
stays fixed throughout the zoom animation; dragging interrupts it immediately.

## Nesting settings

Open the settings button in the tool rail or **Optimizer → Advanced settings**.
Five sections restore the original 16 controls: display units, spacing, all 1–32
rotation counts, placement strategy, curve/endpoint tolerances, rough approximation,
CPU workers, SVG scale, DXF import/export units, shared cuts, cutting-time weight,
population size, and mutation rate. Each has a visual explanation and practical
examples. Settings use an Apply/Cancel draft, support Reset and Undo, and save with
the project. Import settings apply to subsequent imports.

Search starts with area-sorted, reversed, and shuffled part orders, then uses
crossover, mutation, and elitism. Ranking prioritizes placed parts, fewer sheets,
and then compactness. Refinement preserves the incumbent layout, and export runs
a final validation of the original contours. Choose
an evaluation budget or search until stopped, using up to eight parallel workers.
See the [original-settings comparison](docs/settings-parity.md) for the complete
mapping, compatibility details, and test coverage.

## Optimizer measurements

Run the bundled sample without opening a window and print a JSON benchmark:

```sh
cargo run -p deepnest-app -- --benchmark
cargo run -p deepnest-app -- --benchmark path/to/job.deepnest
```

Use finite budgets for reproducible runs. The report includes coverage, sheet
count, utilization, fitness, timing, cache query counts, and improvement history.
The optimizer currently runs on CPU; GPU rendering and GPU computation are
separate capabilities. See [the staged engine roadmap](docs/optimizer-roadmap.md).

## Design system

The native UI follows [shadcn's semantic token convention](https://ui.shadcn.com/docs/theming),
implemented in Rust rather than CSS:

- `crates/app/src/tokens.rs`: semantic colors, typography, spacing, corner radii,
  and common control dimensions.
- `crates/app/src/style.rs`: shared button variants, inputs, floating surfaces,
  scrollbars, and selects.
- `crates/app/src/motion.rs`: centrally chosen transitions for layout and camera
  movement. Layout uses a Time spring with **0.20 s visual duration / 0.25 bounce**.
- `crates/motion`: reusable, frame-rate-independent Time springs, physical
  springs (stiffness/damping/mass), and cubic Bézier easing.

Spring interruptions preserve current velocity. The project menu uses a clipped
reveal, keeping its content stable while opening or closing. Job assets stays
visible at desktop widths and only collapses for compact windows.
Tooltips share one animated surface: the first waits 350 ms, then neighboring
hints slide into place without repeating the delay. Camera movement uses a
non-bouncy spring. Direct manipulation stays immediate. Frame updates stop when
animations settle. On macOS the app reads the system Reduce Motion preference
once when first needed; when enabled, transitions settle immediately.

See [the design-system guide](docs/design-system.md) for token and transition examples.

## Code layout

```text
crates/app/       Iced UI, SVG/DXF import/export, project model, cancellable worker
crates/motion/    Analytic spring and Bézier transition engine
crates/nest/      Native NFP geometry, polygon operations, and placement
resources/macos/ Application bundle metadata and icon
scripts/         Local bundle creation
```

The new worker expands outlines for the requested part gap, uses a per-job NFP
cache, evaluates genetic populations, and validates candidates for overlap and
sheet bounds before displaying or exporting them. It checks cancellation between
placement operations. NFP convolution includes every boundary vertex to avoid
false holes for curved outlines.

## Scope and limitations

- Local SVG import supports closed vector paths, common shapes, viewBox/units,
  transforms, and contained holes. Curves use the configured tolerance (0.1 mm by default);
  exports use those polygonal outlines, not the original Bézier curves.
- Contours that cannot be closed within the endpoint tolerance, text, raster
  images, and clipped/masked/filtered groups are skipped and reported. Convert
  these to plain closed paths before importing.
- DXF import supports ASCII and binary DXF with 2D LINE/ARC chains, CIRCLE,
  LWPOLYLINE/POLYLINE (including bulges), ELLIPSE, and nested single block INSERTs
  with scaling and rotation. Declared units convert to millimeters. Unitless
  drawings are treated as millimeters, with a visible import notice. Endpoint
  joining and curve tolerances are configurable (0.01 mm and 0.1 mm by default).
- DXF splines, text, hatches, meshes, tilted 3D geometry, external references,
  and array INSERTs are skipped and reported. Convert those objects to plain
  2D polylines first. Disconnected open contours are not turned into cut shapes.
- Stock is rectangular. Exports contain cut geometry and holes, without a stock
  outline or machine-specific G-code. DXF export writes polygonal outlines or
  shared cut segments when shared-line cutting is enabled with zero part gap.
- CDR conversion, cloud accounts, and cloud export are not part of the Iced app.
- The search is heuristic, not a guarantee of an optimal arrangement.
- Tests cover SVG/DXF units and holes, binary DXF, bulges, block transforms,
  connected contours, export round trips, shared tooltip timing, project validation,
  cursor-anchored zoom, stale-layout invalidation, save revision handling,
  nesting collisions/gaps, curved NFPs, and the motion math.

Original Deepnest © Jack Qiao — [deepnest.io](https://deepnest.io) — based on
[SVGNest](https://github.com/Jack000/SVGnest). Original attribution is preserved
in [LICENSE](LICENSE).
