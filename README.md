# opennest-rs

**Deepnest** — the nesting tool for laser cutters and other CNC tools — ported
from **Electron** to **Tauri**.

This workspace started as a Tauri + Vue scaffold. The Vue scaffold has been
removed and replaced with Deepnest's actual application, running on a Tauri
shell instead of Electron.

## What was migrated

| Concern | Electron (Deepnest) | Tauri (opennest-rs) |
|---|---|---|
| App shell | Electron main process (`main.js`) | `src-tauri` (Rust) |
| UI | `main/` HTML/JS (Ractive, interact.js) | `frontend/` — same code, unchanged |
| Native NFP geometry | C++ N-API addon (`minkowski.cc`, Boost.Polygon) | **Rust → WebAssembly** (`crates/nfp`) |
| `require('electron')`, `fs`, `path`, … | Node integration in the renderer | `frontend/electron-shim.js` |
| Renderer ↔ worker IPC | `ipcMain` routing + hidden `BrowserWindow` | Tauri broadcast events + hidden window |
| Settings | `electron-settings` | `localStorage` (via the shim) |
| File dialogs | `electron.remote.dialog` | `tauri-plugin-dialog` |

### The NFP engine (`crates/nfp`)

Deepnest's speed-critical No-Fit-Polygon math was a native C++ addon — it
cannot load in a Tauri webview. It has been **reimplemented in Rust** and
compiled to WebAssembly, so it still runs synchronously inside the page like
the original `.node` addon. See `crates/nfp/src/lib.rs`.

The port is verified against the original algorithm for convex, concave, and
holed inputs. It uses the `geo` crate's boolean ops instead of Boost.Polygon,
working directly in `f64` (the C++ version had to scale to integers).

### The compatibility shim (`frontend/electron-shim.js`)

Loaded as the first script in `index.html` and `background.html`, it installs a
`window.require` that returns Tauri-backed implementations of every Node/
Electron module Deepnest uses. Where Electron offered a *synchronous* Node API
that Tauri only does asynchronously, the shim degrades gracefully (see comments
in the file).

## Project layout

```
opennest-rs/
├── frontend/            Deepnest's UI (the Tauri frontend, served statically)
│   ├── electron-shim.js   Electron→Tauri compatibility layer
│   ├── nfp/               NFP wasm engine output (nfp.js + nfp_bg.wasm)
│   ├── index.html         main window
│   ├── background.html    hidden nesting-worker window
│   └── util/ img/ font/ … unchanged Deepnest assets
├── crates/nfp/          Rust NFP geometry engine (compiles to wasm)
├── src-tauri/           Tauri Rust backend (file IO commands, plugins)
└── scripts/build-nfp.sh rebuilds the wasm engine
```

## Building & running

### Prerequisites

- Rust + Cargo
- Node.js (for the Tauri CLI) — `npm install`
- The wasm toolchain, only if you change `crates/nfp`:
  - `rustup target add wasm32-unknown-unknown`
  - `cargo install wasm-bindgen-cli --version 0.2.100`
- **Platform webview libraries:**
  - **Windows** — WebView2 (preinstalled on Windows 10/11). Builds out of the box.
  - **Linux / WSL** — install the WebKitGTK dev packages:
    ```
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
      libayatana-appindicator3-dev librsvg2-dev patchelf \
      build-essential curl wget file libssl-dev libxdo-dev
    ```

### Run

```
npm install
npm run dev      # tauri dev
npm run build    # tauri build (installer/bundle)
```

The NFP wasm engine is already built into `frontend/nfp/`. Rebuild it only
after editing `crates/nfp`:

```
npm run build:nfp
```

## Status & known limitations

**Working / ported:** project structure, the NFP geometry engine (verified),
the Electron→Tauri shim, settings, file dialogs, file read/write, the
main↔worker event bus, the hidden nesting-worker window.

**Degrades gracefully (needs deepnest.io's backend, which is not part of this
project):**

- **DXF/CDR import** — Deepnest uploaded these to a conversion server. SVG
  import works locally; DXF/CDR shows a "server unavailable" message.
- **Cloud export** (DXF/G-code via deepnest.io) — same. **SVG export works**
  (written locally).
- **Auth0 login / accounts** — stubbed.

**Not yet verified end-to-end:** the full Deepnest app is ~10k lines of
Electron-era renderer code; the Tauri shell, shim, and wasm engine compile and
the geometry is tested, but a complete click-through of every feature inside a
running webview has not been done here (the WSL environment used for the port
lacks the Linux webview libraries). Expect to iterate on runtime details —
worker script paths and the occasional sync-vs-async edge — on first run.

---

Original Deepnest © Jack Qiao — <https://deepnest.io> — based on
[SVGNest](https://github.com/Jack000/SVGnest).
