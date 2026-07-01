mod error;
mod steam;
mod downloader;
mod theme;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::load_launcher_theme,
            commands::load_launcher_settings,
            commands::save_launcher_profile,
            commands::load_git_metadata,
            commands::download_and_launch_version,
            commands::minimize_main_window,
            commands::close_main_window,
            commands::kill_background_processes,
            commands::detect_installed_games,
            commands::check_for_csgo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
