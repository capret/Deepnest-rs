// opennest-rs — Tauri backend for the Deepnest nesting app.
//
// The renderer talks to the system through the JS shim
// (frontend/electron-shim.js). The Rust side provides:
//   * file-IO commands so the shim's `fs` can read/write dialog-picked paths,
//   * `run_nest`, which runs the placement algorithm (crate `nest`) on a
//     worker thread so the UI never blocks,
//   * `stop_nest`, which cancels in-flight nests.

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tauri::Emitter;

struct NestState {
    /// Persistent NFP cache, shared across `run_nest` calls and the worker
    /// threads running concurrent nests (`nest::NfpCache` is thread-safe).
    cache: Arc<nest::NfpCache>,
    /// Bumped by `stop_nest`. An in-flight nest whose start snapshot no longer
    /// matches aborts early and its result is discarded.
    epoch: Arc<AtomicU64>,
}

/// Read a UTF-8 text file. Backs `fs.readFile` in the shim (SVG import, etc.).
#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
}

/// Write a UTF-8 text file. Backs `fs.writeFile`/`writeFileSync` in the shim
/// (SVG/DXF/G-code export).
#[tauri::command]
fn write_file(path: String, contents: String) -> Result<(), String> {
    fs::write(&path, contents).map_err(|e| format!("{path}: {e}"))
}

/// Cancel every in-flight nest — the renderer's "Stop" button. Running worker
/// threads notice the bumped epoch, stop computing, and discard their result
/// so nothing more is rendered after Stop.
#[tauri::command]
fn stop_nest(state: tauri::State<'_, NestState>) {
    state.epoch.fetch_add(1, Ordering::SeqCst);
}

/// Run the nesting placement for one genetic-algorithm individual.
///
/// Returns immediately; the placement runs on a worker thread so the UI
/// thread never blocks. Progress is emitted as `background-progress` (rate-
/// limited to keep IPC off the UI thread) and the finished layout as
/// `background-response` — the events Deepnest's renderer already listens for.
#[tauri::command]
fn run_nest(input: nest::NestInput, app: tauri::AppHandle, state: tauri::State<'_, NestState>) {
    let cache = state.cache.clone();
    let epoch = state.epoch.clone();
    let start_epoch = epoch.load(Ordering::SeqCst);

    thread::spawn(move || {
        let index = input.index as i64;
        let progress_app = app.clone();
        let cancel_epoch = epoch.clone();
        // Rate-limit progress events: a few per second is plenty for the bar
        // and keeps a flood of IPC messages off the renderer's UI thread.
        let mut last_emit = Instant::now();
        let mut first = true;

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            nest::run(
                input,
                &cache,
                |progress| {
                    let finished = progress < 0.0;
                    if finished || first || last_emit.elapsed() >= Duration::from_millis(60) {
                        first = false;
                        last_emit = Instant::now();
                        let _ = progress_app.emit(
                            "background-progress",
                            serde_json::json!({ "index": index, "progress": progress }),
                        );
                    }
                },
                || cancel_epoch.load(Ordering::SeqCst) != start_epoch,
            )
        }));

        // A Stop pressed mid-run makes this result stale — drop it silently.
        if epoch.load(Ordering::SeqCst) != start_epoch {
            return;
        }
        match outcome {
            Ok(result) => {
                let _ = app.emit("background-response", &result);
            }
            Err(_) => {
                // Don't let a panic stall the GA: report a finished individual
                // with a deliberately terrible fitness so evolution moves on.
                eprintln!("[run_nest] nesting panicked for individual {index}");
                let _ = app.emit(
                    "background-response",
                    serde_json::json!({
                        "index": index,
                        "placements": [],
                        "fitness": 1.0e12,
                        "area": 0.0,
                        "mergedLength": 0.0
                    }),
                );
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(NestState {
            cache: Arc::new(nest::NfpCache::default()),
            epoch: Arc::new(AtomicU64::new(0)),
        })
        .invoke_handler(tauri::generate_handler![
            read_file, write_file, run_nest, stop_nest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
