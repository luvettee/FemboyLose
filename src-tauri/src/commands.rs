use sha2::digest::typenum::Or;
use tauri::AppHandle;
use tauri::Manager;
use crate::error::LauncherError;
use crate::steam;
use crate::downloader;
use crate::theme;
use sysinfo::System;

#[tauri::command]
pub async fn load_launcher_theme() -> Result<theme::LauncherTheme, LauncherError> {
    theme::load_launcher_theme().await
}

#[tauri::command]
pub fn minimize_main_window(app: AppHandle) -> Result<(), LauncherError> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| LauncherError::System("main window was not found".to_string()))?;
    window
        .minimize()
        .map_err(|error| LauncherError::System(format!("failed to minimize main window: {error}")))
}

#[tauri::command]
pub fn close_main_window(app: AppHandle) -> Result<(), LauncherError> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| LauncherError::System("main window was not found".to_string()))?;
    window
        .close()
        .map_err(|error| LauncherError::System(format!("failed to close main window: {error}")))
}

#[tauri::command]
pub async fn load_launcher_settings() -> Result<theme::LauncherSettings, LauncherError> {
    theme::read_launcher_settings().await
}

#[tauri::command]
pub async fn save_launcher_profile(
    username: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<theme::LauncherSettings, LauncherError> {
    theme::save_launcher_profile(username, avatar_bytes).await
}

#[tauri::command]
pub async fn load_git_metadata() -> Result<downloader::LauncherGitMetadata, LauncherError> {
    downloader::load_git_metadata().await
}

#[tauri::command]
pub async fn download_and_launch_version(
    tag: String,
    config_id: Option<i32>,
    appid: i32,
    auto_launch: bool,
) -> Result<(), LauncherError> {
    downloader::download_and_launch_version(tag, config_id, appid, auto_launch).await
}

#[tauri::command]
pub fn kill_background_processes() -> Result<(), LauncherError> {
    downloader::kill_background_processes()
}

#[tauri::command]
pub fn detect_installed_games() -> Result<steam::InstalledGames, LauncherError> {
    Ok(steam::InstalledGames {
        cs2_legacy_branch: steam::find_game_install_path("Counter-Strike Global Offensive").is_some(),
        csgo_standalone: steam::find_game_install_path("csgo legacy").is_some(),
    })
}

#[tauri::command]
pub fn check_for_csgo() -> bool {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let x = sys.processes_by_exact_name(std::ffi::OsStr::new("csgo.exe"))
        .next()
        .is_some(); x // i fucking hate this shit i hope this works
}
