# opennest-rs

**Deepnest** — the nesting tool for laser cutters and other CNC tools —
ported from **Electron** to **Tauri**, with its entire nesting computation
rewritten in native **Rust**.

This workspace began as a Tauri + Vue scaffold; the scaffold was removed and
replaced with Deepnest's application running on a Tauri shell.

## What was migrated

| Concern | Electron (Deepnest) | Tauri (opennest-rs) |
|---|---|---|
| App shell | Electron main process (`main.js`) | `src-tauri` (Rust) |
| UI | `main/` HTML/JS (Ractive, interact.js) | `frontend/` — same code, unchanged |
| Nesting computation | hidden `BrowserWindow` running `background.js` | **native Rust** (`crates/nest`) |
| NFP geometry | C++ N-API addon (`minkowski.cc`, Boost.Polygon) | Rust (`crates/nest/src/nfp.rs`) |
| Polygon clipping | in-page ClipperLib | Rust `geo` boolean ops (`crates/nest/src/clip.rs`) |
| `require('electron')`, `fs`, `path`, … | Node integration in the renderer | `frontend/electron-shim.js` |
| Renderer ↔ worker IPC | `ipcMain` routing + worker window | one `run_nest` Tauri command |
| Settings | `electron-settings` | `localStorage` (via the shim) |
| File dialogs | `electron.remote.dialog` | `tauri-plugin-dialog` |

### The nesting engine (`crates/nest`)

Deepnest ran its placement algorithm in a hidden Electron window
(`background.js`), leaning on a native C++ addon for No-Fit-Polygon math and
on ClipperLib for polygon booleans. None of that survives in a Tauri webview,
so the **whole computation was ported to a native Rust crate**:

- `nfp.rs` — No-Fit-Polygon via Minkowski-sum convolution (was `minkowski.cc`).
- `clip.rs` — polygon union/difference via the `geo` crate (was ClipperLib).
- `place.rs` — `placeParts`, `getInnerNfp`/`getOuterNfp`, line merging.
- `geom.rs` — geometry primitives (area, bounds, rotation, convex hull).
- `lib.rs` — the `run` entry point, plus unit tests.

It is plain native Rust (no webview/wasm toolchain), so the algorithm is
**unit-tested directly** — `cargo test` covers NFP, boolean ops, and a full
placement.

The renderer calls it through the `run_nest` command, which returns
immediately and runs the placement on a **worker thread** — the UI thread
never blocks. Progress and the finished layout come back as
`background-progress` / `background-response` events. The NFP cache is
thread-safe, so the genetic algorithm's individuals are evaluated **in
parallel** (up to `config.threads` at once).

### The compatibility shim (`frontend/electron-shim.js`)

Loaded as the first script in `index.html`, it installs a `window.require`
returning Tauri-backed implementations of every Node/Electron module Deepnest
uses. It also intercepts the renderer's `background-start` message and turns
it into a `run_nest` invoke — so `deepnest.js` itself is unchanged.

## Project layout

```
opennest-rs/
├── frontend/            Deepnest's UI (the Tauri frontend, served statically)
│   ├── electron-shim.js   Electron→Tauri compatibility layer
│   ├── deepnest.js        Deepnest renderer + genetic algorithm (unchanged)
│   ├── index.html         main window
│   └── util/ img/ font/ … unchanged Deepnest assets
│   └── background.*       original Electron worker — kept for reference, unused
├── crates/nest/         native Rust nesting engine (NFP + placement)
└── src-tauri/           Tauri Rust backend (run_nest + file-IO commands)
```

## Building & running

### Prerequisites

- Rust + Cargo
- Node.js (for the Tauri CLI) — `npm install`
- **Platform webview libraries:**
  - **Windows** — WebView2 (preinstalled on Windows 10/11). Builds out of the box.
  - **Linux / WSL** — install the WebKitGTK dev packages:
    ```
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
      libayatana-appindicator3-dev librsvg2-dev patchelf \
      build-essential curl wget file libssl-dev libxdo-dev
    ```

### Commands

```
npm install
npm run dev        # tauri dev
npm run build      # tauri build (installer/bundle)
npm run test:nest  # cargo test for the nesting engine
```

## Status & known limitations

**Working / ported:** project structure, the full nesting engine (NFP,
boolean clipping, placement, line merging — unit-tested), the Electron→Tauri
shim, settings, file dialogs, file read/write, the `run_nest` command and its
progress events.

**Degrades gracefully (needs deepnest.io's backend, which is not part of this
project):**

- **DXF/CDR import** — Deepnest uploaded these to a conversion server. SVG
  import works locally; DXF/CDR shows a "server unavailable" message.
- **Cloud export** (DXF/G-code via deepnest.io) — same. **SVG export works**.
- **Auth0 login / accounts** — stubbed.

**Faithfulness notes:** the Rust engine is a close port of `background.js`.
Two deliberate departures from the original: `mergedLength` keeps its
short-edge cutoff constant (the original reused one variable via JS `var`
hoisting), and the NFP cache also stores hole-free inner NFPs (a pure speed
win, identical results).

**Not yet verified end-to-end:** the engine is unit-tested and the Tauri
shell/shim compile, but a full click-through inside a running webview has not
been done here (the WSL environment used for the port lacks the Linux webview
libraries). Expect to iterate on runtime details on first run.

---

Original Deepnest © Jack Qiao — <https://deepnest.io> — based on
[SVGNest](https://github.com/Jack000/SVGnest).
