// opennest-rs — Tauri backend for the Deepnest nesting app.
//
// Deepnest's renderer talks to the system mostly through the JS shim
// (frontend/electron-shim.js). The Rust side stays deliberately thin: it
// registers the dialog/opener plugins and exposes two file-IO commands so the
// shim's `fs` can read/write dialog-picked paths anywhere on disk (without the
// fs-plugin's path scoping). Renderer<->worker messaging uses Tauri's
// broadcast events, so no main-process IPC routing is needed here.

use std::fs;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_file, write_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
