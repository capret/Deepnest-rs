// opennest-rs — Tauri backend for the Deepnest nesting app.
//
// The renderer talks to the system through the JS shim
// (frontend/electron-shim.js). The Rust side provides:
//   * file-IO commands so the shim's `fs` can read/write dialog-picked paths,
//   * the `run_nest` command, which runs the entire placement algorithm
//     natively (crate `nest`) — this replaces Deepnest's hidden Electron
//     worker window, its C++ Minkowski addon, and its in-page ClipperLib.

use std::fs;
use std::sync::Mutex;

use tauri::Emitter;

/// Persistent NFP cache, shared across `run_nest` calls (Deepnest kept this in
/// its background window). A Mutex serialises concurrent nests, mirroring the
/// single-worker model of the original.
struct NestState(Mutex<nest::NfpCache>);

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

/// Run the nesting placement for one genetic-algorithm individual.
///
/// This is the Tauri equivalent of Deepnest's `background-start` handler:
/// the shim forwards the renderer's payload here, the placement runs natively,
/// `background-progress` events are emitted during the run, and the result is
/// returned for the shim to deliver as `background-response`.
#[tauri::command]
fn run_nest(
    input: nest::NestInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, NestState>,
) -> Result<nest::NestResult, String> {
    let index = input.index as i64;
    let mut cache = state
        .0
        .lock()
        .map_err(|e| format!("nfp cache lock poisoned: {e}"))?;
    let result = nest::run(input, &mut cache, |progress| {
        let _ = app.emit(
            "background-progress",
            serde_json::json!({ "index": index, "progress": progress }),
        );
    });
    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(NestState(Mutex::new(nest::NfpCache::default())))
        .invoke_handler(tauri::generate_handler![read_file, write_file, run_nest])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
