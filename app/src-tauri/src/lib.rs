//! The Tauri side of the Lockin app.
//! Commands that the Svelte frontend calls with `invoke(...)` go here
//! (https://tauri.app/develop/calling-rust/).

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running the Lockin app");
}
