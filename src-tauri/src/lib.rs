mod commands;
#[cfg(target_os = "linux")]
mod desktop_entry;
pub mod extract;
pub mod games;
pub mod model;
pub mod save;
pub mod unity;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dev builds run from target/: don't register them.
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    if let Err(e) = desktop_entry::ensure() {
        eprintln!("desktop entry: {e}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::game_statuses,
            commands::load_game,
            commands::set_game_path,
            commands::load_lists,
            commands::save_lists,
            commands::save_slots,
            commands::read_save,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
