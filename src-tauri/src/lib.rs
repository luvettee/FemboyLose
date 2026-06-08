use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::Threading::{OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_TERMINATE},
    UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
};
#[cfg(windows)]
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

use base64::Engine as _;
use rmpv::Value;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/luvettee/FemboyLose/releases";
const GITHUB_RELEASE_BY_TAG_URL: &str =
    "https://api.github.com/repos/luvettee/FemboyLose/releases/tags";
const CREATE_NO_WINDOW: u32 = 0x08000000;
const CSGO_WINDOW_CLASS: &str = "Valve001";
const SYNCHRONIZE_ACCESS: u32 = 0x00100000;
const WAIT_OBJECT_0: u32 = 0;
const STYLE_ALPHA: &str = "177670";
const STYLE_BLUE: &str = "177671";
const STYLE_GREEN: &str = "177676";
const STYLE_RED: &str = "177687";
const STYLE_SELECTION_KEY_1: &str = "2090499946";
const STYLE_SELECTION_KEY_2: &str = "993947594";
const STYLE_SELECTION_KEY_3: &str = "277698370";
const BUILTIN_STYLE_BLUE: &str = include_str!("../builtin-styles/Blue.style");
const BUILTIN_STYLE_BLACK: &str = include_str!("../builtin-styles/Black.style");
const BUILTIN_STYLE_LIGHT: &str = include_str!("../builtin-styles/Light.style");

#[derive(Debug, Deserialize)]
struct CloudState {
    #[serde(default)]
    username: String,
    #[serde(default)]
    type7_blob: Option<String>,
    #[serde(default)]
    last_loaded_config_id: Option<i32>,
    last_loaded_style_id: Option<i32>,
    log: Vec<LogEntry>,
}

#[derive(Debug, Deserialize)]
struct LogEntry {
    entry_id: i32,
    entry_type: String,
    name: String,
    #[serde(default)]
    deleted_at: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct ThemeColor {
    css: String,
    hex: String,
    alpha: f64,
}

#[derive(Debug, Serialize)]
struct LauncherTheme {
    source: String,
    variables: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct LauncherSettings {
    username: String,
    avatar_data_url: Option<String>,
    selected_config_id: Option<i32>,
    configs: Vec<ConfigEntry>,
}

#[derive(Debug, Serialize)]
struct ConfigEntry {
    entry_id: i32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    prerelease: bool,
    draft: bool,
    published_at: Option<String>,
    updated_at: String,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Serialize)]
struct LauncherGitMetadata {
    releases: Vec<LauncherVersion>,
    nightlies: Vec<LauncherVersion>,
}

#[derive(Debug, Serialize)]
struct LauncherVersion {
    tag: String,
    name: String,
    changelog: String,
    updated_at: String,
    url: String,
    assets: Vec<LauncherAsset>,
}

#[derive(Debug, Serialize)]
struct LauncherAsset {
    name: String,
    url: String,
    size: u64,
}

#[tauri::command]
fn load_launcher_theme() -> Result<LauncherTheme, String> {
    let cloud = nl_cloud_path()?;
    let state_path = cloud.join("state.json");
    let state_text = fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read {}: {error}", state_path.display()))?;
    let state: CloudState = serde_json::from_str(&state_text)
        .map_err(|error| format!("failed to parse {}: {error}", state_path.display()))?;

    let style_id = state
        .type7_blob
        .as_deref()
        .and_then(extract_style_id_from_type7)
        .or(state.last_loaded_style_id);

    let Some(style_id) = style_id else {
        return Ok(default_theme("built-in style"));
    };

    if let Some((name, style_text)) = builtin_style(style_id) {
        let colors = decode_style_colors(style_text)?;
        return Ok(LauncherTheme {
            source: format!("built-in {name} style"),
            variables: build_launcher_variables(&colors),
        });
    }

    let Some(entry) = state.log.iter().find(|entry| {
        entry.entry_id == style_id && entry.entry_type == "Style" && entry.deleted_at.is_none()
    }) else {
        return Ok(default_theme("style not found"));
    };

    let style_path = cloud.join("styles").join(format!(
        "{}_{}.style",
        entry.entry_id,
        sanitize_filename(&entry.name)
    ));
    let style_text = fs::read_to_string(&style_path)
        .map_err(|error| format!("failed to read {}: {error}", style_path.display()))?;
    let colors = decode_style_colors(&style_text)?;

    Ok(LauncherTheme {
        source: style_path.display().to_string(),
        variables: build_launcher_variables(&colors),
    })
}

#[tauri::command]
fn minimize_main_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window was not found".to_string())?;
    window
        .minimize()
        .map_err(|error| format!("failed to minimize main window: {error}"))
}

#[tauri::command]
fn close_main_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window was not found".to_string())?;
    window
        .close()
        .map_err(|error| format!("failed to close main window: {error}"))
}

#[tauri::command]
fn load_launcher_settings() -> Result<LauncherSettings, String> {
    read_launcher_settings()
}

#[tauri::command]
fn save_launcher_profile(
    username: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<LauncherSettings, String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("profile name cannot be empty".to_string());
    }

    let cloud = nl_cloud_path()?;
    fs::create_dir_all(&cloud)
        .map_err(|error| format!("failed to create {}: {error}", cloud.display()))?;

    let state_path = cloud.join("state.json");
    let state_text = fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read {}: {error}", state_path.display()))?;
    let mut state_value: serde_json::Value = serde_json::from_str(&state_text)
        .map_err(|error| format!("failed to parse {}: {error}", state_path.display()))?;
    let Some(state_object) = state_value.as_object_mut() else {
        return Err("state.json root was not an object".to_string());
    };
    state_object.insert(
        "username".to_string(),
        serde_json::Value::String(username.to_string()),
    );
    let next_state = serde_json::to_string_pretty(&state_value)
        .map_err(|error| format!("failed to serialize {}: {error}", state_path.display()))?;
    fs::write(&state_path, next_state)
        .map_err(|error| format!("failed to write {}: {error}", state_path.display()))?;

    if let Some(bytes) = avatar_bytes {
        if bytes.is_empty() {
            return Err("profile image was empty".to_string());
        }
        let avatar_path = cloud.join("avatar.png");
        fs::write(&avatar_path, bytes)
            .map_err(|error| format!("failed to write {}: {error}", avatar_path.display()))?;
    }

    read_launcher_settings()
}

fn read_launcher_settings() -> Result<LauncherSettings, String> {
    let cloud = nl_cloud_path()?;
    let state_path = cloud.join("state.json");
    let state_text = fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read {}: {error}", state_path.display()))?;
    let state: CloudState = serde_json::from_str(&state_text)
        .map_err(|error| format!("failed to parse {}: {error}", state_path.display()))?;

    let configs = state
        .log
        .iter()
        .filter(|entry| entry.entry_type == "Config" && entry.deleted_at.is_none())
        .map(|entry| ConfigEntry {
            entry_id: entry.entry_id,
            name: entry.name.clone(),
        })
        .collect::<Vec<_>>();

    Ok(LauncherSettings {
        username: state.username,
        avatar_data_url: load_avatar_data_url(&cloud)?,
        selected_config_id: state.last_loaded_config_id,
        configs,
    })
}

fn load_avatar_data_url(cloud: &Path) -> Result<Option<String>, String> {
    let avatar_path = cloud.join("avatar.png");
    if !avatar_path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&avatar_path)
        .map_err(|error| format!("failed to read {}: {error}", avatar_path.display()))?;
    let mime = image_mime_type(&bytes).unwrap_or("image/png");
    Ok(Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

fn image_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

#[tauri::command]
fn load_git_metadata() -> Result<LauncherGitMetadata, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("NeverloseTauriOfficial")
        .build()
        .map_err(|error| format!("failed to create GitHub client: {error}"))?;
    let releases = client
        .get(GITHUB_RELEASES_URL)
        .send()
        .map_err(|error| format!("failed to fetch GitHub releases: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub releases request failed: {error}"))?
        .json::<Vec<GithubRelease>>()
        .map_err(|error| format!("failed to parse GitHub releases: {error}"))?;

    let mut stable = Vec::new();
    let mut nightly = Vec::new();

    for release in releases.into_iter().filter(|release| !release.draft) {
        let version = LauncherVersion {
            name: release.name.unwrap_or_else(|| release.tag_name.clone()),
            tag: release.tag_name,
            changelog: release.body.unwrap_or_default(),
            updated_at: release.published_at.unwrap_or(release.updated_at),
            url: release.html_url,
            assets: release
                .assets
                .into_iter()
                .map(|asset| LauncherAsset {
                    name: asset.name,
                    url: asset.browser_download_url,
                    size: asset.size,
                })
                .collect(),
        };

        if release.prerelease {
            nightly.push(version);
        } else {
            stable.push(version);
        }
    }

    Ok(LauncherGitMetadata {
        releases: stable,
        nightlies: nightly,
    })
}

#[tauri::command]
fn download_and_launch_version(
    tag: String,
    config_id: Option<i32>,
    appid: i32,
) -> Result<(), String> {
    if tag.trim().is_empty() || tag == "Unavailable" {
        return Err("no release version is selected".to_string());
    }

    let client = github_client()?;
    let release = client
        .get(format!("{GITHUB_RELEASE_BY_TAG_URL}/{tag}"))
        .send()
        .map_err(|error| format!("failed to fetch GitHub release {tag}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub release {tag} request failed: {error}"))?
        .json::<GithubRelease>()
        .map_err(|error| format!("failed to parse GitHub release {tag}: {error}"))?;

    let install_dir = version_install_dir(&tag)?;
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create {}: {error}", install_dir.display()))?;

    download_asset(&client, &release, "neverlose.dll", &install_dir)?;
    download_asset(&client, &release, "neverlose-server.exe", &install_dir)?;
    download_asset(&client, &release, "injector.exe", &install_dir)?;

    let cloud_dir = launcher_cloud_dir()?;
    fs::create_dir_all(&cloud_dir)
        .map_err(|error| format!("failed to create {}: {error}", cloud_dir.display()))?;

    spawn_server_hidden(
        &install_dir.join("neverlose-server.exe"),
        &install_dir,
        &cloud_dir,
        config_id,
    )?;
    restart_csgo(appid)?;
    spawn_hidden(&install_dir.join("injector.exe"), &install_dir)?;

    Ok(())
}

fn github_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("NeverloseTauriOfficial")
        .build()
        .map_err(|error| format!("failed to create GitHub client: {error}"))
}

fn version_install_dir(tag: &str) -> Result<PathBuf, String> {
    let appdata =
        std::env::var("APPDATA").map_err(|error| format!("APPDATA is not available: {error}"))?;
    Ok(Path::new(&appdata)
        .join("neverlose")
        .join("bin")
        .join(sanitize_path_segment(tag)))
}

fn launcher_cloud_dir() -> Result<PathBuf, String> {
    if let Some(game_dir) = csgo_install_dir() {
        return Ok(game_dir.join("nl_cloud"));
    }

    let appdata =
        std::env::var("APPDATA").map_err(|error| format!("APPDATA is not available: {error}"))?;
    Ok(Path::new(&appdata).join("neverlose").join("nl_cloud"))
}

#[cfg(windows)]
fn csgo_install_dir() -> Option<PathBuf> {
    let local_machine = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = local_machine
        .open_subkey("SOFTWARE\\WOW6432Node\\Valve\\cs2")
        .ok()?;
    let install_path: String = key.get_value("installpath").ok()?;
    let path = PathBuf::from(install_path);

    path.join("csgo.exe").exists().then_some(path)
}

#[cfg(not(windows))]
fn csgo_install_dir() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn steam_install_dir() -> Option<PathBuf> {
    let local_machine = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = local_machine
        .open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        .ok()?;
    let install_path: String = key.get_value("InstallPath").ok()?;
    let path = PathBuf::from(install_path);

    path.join("steam.exe").exists().then_some(path)
}

#[cfg(not(windows))]
fn steam_install_dir() -> Option<PathBuf> {
    None
}

fn download_asset(
    client: &reqwest::blocking::Client,
    release: &GithubRelease,
    asset_name: &str,
    install_dir: &Path,
) -> Result<(), String> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(asset_name))
        .ok_or_else(|| format!("release {} is missing {asset_name}", release.tag_name))?;
    let target = install_dir.join(asset_name);
    let temp = install_dir.join(format!("{asset_name}.download"));
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .map_err(|error| format!("failed to download {asset_name}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("download for {asset_name} failed: {error}"))?
        .bytes()
        .map_err(|error| format!("failed to read {asset_name}: {error}"))?;

    fs::write(&temp, bytes)
        .map_err(|error| format!("failed to write {}: {error}", temp.display()))?;
    if target.exists() {
        fs::remove_file(&target)
            .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
    }
    fs::rename(&temp, &target)
        .map_err(|error| format!("failed to move {} into place: {error}", target.display()))?;

    Ok(())
}

fn spawn_hidden(exe: &Path, working_dir: &Path) -> Result<(), String> {
    let mut command = Command::new(exe);
    command.current_dir(working_dir);

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch {}: {error}", exe.display()))
}

fn spawn_server_hidden(
    exe: &Path,
    working_dir: &Path,
    cloud_dir: &Path,
    config_id: Option<i32>,
) -> Result<(), String> {
    let mut command = Command::new(exe);
    command.current_dir(working_dir);
    command.env("NL_CLOUD_PATH", cloud_dir);
    if let Some(config_id) = config_id {
        command.args(["--boot-config", &config_id.to_string()]);
    }

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch {}: {error}", exe.display()))
}

#[tauri::command]
#[cfg(windows)]
fn kill_background_processes() -> Result<(), String> {
    Command::new("taskkill")
        .args(["/im", "injector.exe", "/f"])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to kill injector: {error}"))?;

    Command::new("taskkill")
        .args(["/im", "neverlose-server.exe", "/f"])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to kill neverlose server: {error}"))?;

    Ok(())
}

#[cfg(windows)]
fn restart_csgo(appid: i32) -> Result<(), String> {
    close_csgo_if_running()?;

    let steam_dir =
        steam_install_dir().ok_or_else(|| "failed to find Steam install path".to_string())?;
    let steam = steam_dir.join("steam.exe");
    
    let protocol_string = match appid {
        730 => "steam://launch/730/option1".to_string(),
        _ => format!("steam://launch/{}/dialog", appid),
    };
    //fix: now doesnt need the pop up

    Command::new(&steam)
        .args([&protocol_string, "-steam", "-insecure", "-novid"])
        .current_dir(&steam_dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch {}: {error}", steam.display()))
}

#[cfg(not(windows))]
fn restart_csgo() -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn close_csgo_if_running() -> Result<(), String> {
    let Some(window) = find_csgo_window() else {
        return Ok(());
    };

    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(window, &mut process_id);
    }

    unsafe {
        let _ = PostMessageW(window, WM_CLOSE, 0, 0);
    }

    if process_id == 0 {
        std::thread::sleep(std::time::Duration::from_millis(1400));
        return Ok(());
    }

    let process = unsafe { OpenProcess(SYNCHRONIZE_ACCESS | PROCESS_TERMINATE, 0, process_id) };
    if process.is_null() {
        std::thread::sleep(std::time::Duration::from_millis(1400));
        return Ok(());
    }

    let closed = unsafe { WaitForSingleObject(process, 3500) == WAIT_OBJECT_0 };
    if !closed {
        unsafe {
            TerminateProcess(process, 0);
            let _ = WaitForSingleObject(process, 2000);
        }
    }

    unsafe {
        CloseHandle(process);
    }

    std::thread::sleep(std::time::Duration::from_millis(700));
    Ok(())
}

#[cfg(windows)]
fn find_csgo_window() -> Option<HWND> {
    let class_name = wide_null(CSGO_WINDOW_CLASS);
    let window = unsafe { FindWindowW(class_name.as_ptr(), std::ptr::null()) };
    (!window.is_null()).then_some(window)
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn builtin_style(style_id: i32) -> Option<(&'static str, &'static str)> {
    match style_id {
        0 => Some(("Blue", BUILTIN_STYLE_BLUE)),
        1 => Some(("Black", BUILTIN_STYLE_BLACK)),
        2 => Some(("Light", BUILTIN_STYLE_LIGHT)),
        _ => None,
    }
}

fn extract_style_id_from_type7(blob_b64: &str) -> Option<i32> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob_b64.trim())
        .ok()?;
    let value = rmpv::decode::read_value(&mut &bytes[..]).ok()?;
    let selected = map_get(&value, STYLE_SELECTION_KEY_1)
        .and_then(|value| map_get(value, STYLE_SELECTION_KEY_2))
        .and_then(|value| map_get(value, STYLE_SELECTION_KEY_3))?;

    selected
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Map(entries) = value else {
        return None;
    };

    entries.iter().find_map(|(entry_key, entry_value)| {
        if entry_key.as_str()? == key {
            Some(entry_value)
        } else {
            None
        }
    })
}

fn nl_cloud_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("NL_CLOUD_PATH") {
        return Ok(PathBuf::from(path));
    }

    launcher_cloud_dir()
}

fn decode_style_colors(style_text: &str) -> Result<Vec<ThemeColor>, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(style_text.trim())
        .map_err(|error| format!("style base64 decode failed: {error}"))?;
    let value = rmpv::decode::read_value(&mut &bytes[..])
        .map_err(|error| format!("style MessagePack decode failed: {error}"))?;
    let Value::Array(items) = value else {
        return Err("style payload was not an array".to_string());
    };

    items
        .iter()
        .map(decode_style_color)
        .collect::<Result<Vec<_>, _>>()
}

fn decode_style_color(value: &Value) -> Result<ThemeColor, String> {
    let Value::Map(entries) = value else {
        return Err("style color entry was not a map".to_string());
    };

    let alpha = style_channel(entries, STYLE_ALPHA)?;
    let red = style_channel(entries, STYLE_RED)?;
    let green = style_channel(entries, STYLE_GREEN)?;
    let blue = style_channel(entries, STYLE_BLUE)?;
    let red_u8 = channel_to_u8(red);
    let green_u8 = channel_to_u8(green);
    let blue_u8 = channel_to_u8(blue);
    let alpha_u8 = channel_to_u8(alpha);

    Ok(ThemeColor {
        css: format!(
            "rgba({}, {}, {}, {:.3})",
            red_u8,
            green_u8,
            blue_u8,
            alpha.clamp(0.0, 1.0)
        ),
        hex: format!("#{red_u8:02X}{green_u8:02X}{blue_u8:02X}{alpha_u8:02X}"),
        alpha,
    })
}

fn style_channel(entries: &[(Value, Value)], key: &str) -> Result<f64, String> {
    entries
        .iter()
        .find_map(|(entry_key, value)| {
            let entry_key = entry_key.as_str()?;
            if entry_key == key {
                value.as_f64()
            } else {
                None
            }
        })
        .ok_or_else(|| format!("style color entry was missing channel {key}"))
}

fn channel_to_u8(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn color(colors: &[ThemeColor], index: usize, fallback: &str) -> String {
    colors
        .get(index)
        .map(|color| color.css.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn alpha_color(colors: &[ThemeColor], index: usize, fallback: &str, alpha: f64) -> String {
    let Some(color) = colors.get(index) else {
        return fallback.to_string();
    };

    let hex = color.hex.trim_start_matches('#');
    if hex.len() < 6 {
        return fallback.to_string();
    }

    let Ok(red) = u8::from_str_radix(&hex[0..2], 16) else {
        return fallback.to_string();
    };
    let Ok(green) = u8::from_str_radix(&hex[2..4], 16) else {
        return fallback.to_string();
    };
    let Ok(blue) = u8::from_str_radix(&hex[4..6], 16) else {
        return fallback.to_string();
    };

    format!("rgba({red}, {green}, {blue}, {:.3})", alpha.clamp(0.0, 1.0))
}

fn opaque_color(colors: &[ThemeColor], index: usize, fallback: &str) -> String {
    alpha_color(colors, index, fallback, 1.0)
}

fn build_launcher_variables(colors: &[ThemeColor]) -> HashMap<String, String> {
    let mut variables = HashMap::new();

    variables.insert(
        "--nl-text".to_string(),
        color(colors, 0, "rgba(255, 255, 255, 0.88)"),
    );
    variables.insert(
        "--nl-disabled-text".to_string(),
        color(colors, 1, "rgba(255, 255, 255, 0.38)"),
    );
    variables.insert("--nl-active-text".to_string(), color(colors, 2, "#FFFFFF"));
    variables.insert(
        "--nl-small-text".to_string(),
        color(colors, 3, "rgba(255, 255, 255, 0.575)"),
    );
    variables.insert(
        "--nl-sidebar-text".to_string(),
        color(colors, 4, "rgba(255, 255, 255, 0.589)"),
    );
    variables.insert("--nl-logo".to_string(), color(colors, 5, "white"));
    variables.insert("--nl-sidebar-bg".to_string(), color(colors, 6, "#010306"));
    variables.insert(
        "--nl-popup-bg".to_string(),
        color(colors, 8, "rgba(7, 12, 19, 0.65)"),
    );
    variables.insert("--nl-main-bg".to_string(), color(colors, 9, "#010306"));
    variables.insert(
        "--nl-main-bg-opaque".to_string(),
        opaque_color(colors, 9, "#010306"),
    );
    variables.insert("--nl-preview-bg".to_string(), color(colors, 10, "#03080f"));
    variables.insert(
        "--nl-border".to_string(),
        color(colors, 11, "rgba(255, 255, 255, 0.075)"),
    );
    variables.insert("--nl-frame-bg".to_string(), color(colors, 12, "#03080f"));
    variables.insert(
        "--nl-frame-active-bg".to_string(),
        color(colors, 13, "rgba(7, 12, 19, 0.72)"),
    );
    variables.insert(
        "--nl-text-preview".to_string(),
        color(colors, 14, "rgba(247, 245, 255, 0.8)"),
    );
    variables.insert(
        "--nl-window-title-bg".to_string(),
        color(colors, 15, "rgba(4, 8, 13, 0.96)"),
    );
    variables.insert(
        "--nl-active-window-title".to_string(),
        color(colors, 16, "rgba(255, 255, 255, 0.88)"),
    );
    variables.insert("--nl-spinner".to_string(), color(colors, 40, "#aab6ff"));
    variables.insert("--nl-block-bg".to_string(), color(colors, 41, "#03080f"));
    variables.insert(
        "--nl-block-bg-opaque".to_string(),
        opaque_color(colors, 41, "#03080f"),
    );
    variables.insert(
        "--nl-sidebar-selection".to_string(),
        color(colors, 42, "rgba(255, 255, 255, 0.08)"),
    );
    variables.insert(
        "--nl-logo-back".to_string(),
        color(colors, 44, "rgba(255, 255, 255, 0.18)"),
    );
    variables.insert("--nl-button".to_string(), color(colors, 28, "#3f66f5"));
    variables.insert(
        "--nl-button-active".to_string(),
        color(colors, 29, "#486cee"),
    );
    variables.insert(
        "--nl-button-active-text".to_string(),
        color(colors, 30, "rgba(255, 255, 255, 0.9)"),
    );
    variables.insert("--nl-link".to_string(), color(colors, 31, "#626be6"));
    variables.insert("--nl-link-active".to_string(), color(colors, 32, "white"));
    variables.insert(
        "--nl-selection".to_string(),
        color(colors, 34, "rgba(16, 31, 49, 0.78)"),
    );
    variables.insert(
        "--nl-separator".to_string(),
        color(colors, 35, "rgba(255, 255, 255, 0.045)"),
    );
    variables.insert(
        "--nl-shadow".to_string(),
        color(colors, 39, "rgba(0, 0, 0, 0.52)"),
    );
    variables.insert(
        "--nl-shadow-soft".to_string(),
        alpha_color(colors, 39, "rgba(0, 0, 0, 0.48)", 0.48),
    );

    variables
}

fn default_theme(source: &str) -> LauncherTheme {
    LauncherTheme {
        source: source.to_string(),
        variables: HashMap::new(),
    }
}

fn sanitize_filename(name: &str) -> String {
    let value: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();
    let value = value.trim_end_matches(|character: char| {
        character == '.' || character == ' ' || character.is_control()
    });
    if value.is_empty() {
        "unnamed".to_string()
    } else {
        value.to_string()
    }
}

fn sanitize_path_segment(name: &str) -> String {
    let value = sanitize_filename(name);
    if value == "." || value == ".." {
        "version".to_string()
    } else {
        value
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_launcher_theme,
            load_launcher_settings,
            save_launcher_profile,
            load_git_metadata,
            download_and_launch_version,
            minimize_main_window,
            close_main_window,
            kill_background_processes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
