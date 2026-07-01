use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::LauncherError;
use sysinfo::{System};

#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::Threading::{OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_TERMINATE},
    UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
};

const CSGO_WINDOW_CLASS: &str = "Valve001";
const SYNCHRONIZE_ACCESS: u32 = 0x00100000;
const WAIT_OBJECT_0: u32 = 0;

#[derive(serde::Serialize)]
pub struct InstalledGames {
    pub cs2_legacy_branch: bool,
    pub csgo_standalone: bool,
}

#[cfg(windows)]
pub fn get_steam_install_path() -> Option<PathBuf> {
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam") {
        if let Ok(steam_path) = key.get_value::<String, _>("SteamPath") {
            return Some(PathBuf::from(steam_path));
        }
    }
    let hklm = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\Wow6432Node\\Valve\\Steam") {
        if let Ok(steam_path) = key.get_value::<String, _>("InstallPath") {
            return Some(PathBuf::from(steam_path));
        }
    }
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
        if let Ok(steam_path) = key.get_value::<String, _>("InstallPath") {
            return Some(PathBuf::from(steam_path));
        }
    }
    None
}

#[cfg(not(windows))]
pub fn get_steam_install_path() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
pub fn steam_install_dir() -> Option<PathBuf> {
    let path = get_steam_install_path()?;
    path.join("steam.exe").exists().then_some(path)
}

#[cfg(not(windows))]
pub fn steam_install_dir() -> Option<PathBuf> {
    None
}

pub fn find_game_install_path(game_name: &str) -> Option<PathBuf> {
    if let Ok(cur) = std::env::current_dir() {
        let exe_check = cur.join("csgo.exe");
        if exe_check.exists() {
            return Some(cur);
        }
    }

    let steam_path = get_steam_install_path()?;

    let check_path = |lib: &Path| -> Option<PathBuf> {
        let full_path = lib.join("steamapps").join("common").join(game_name);
        let exe_check = full_path.join("csgo.exe");
        if exe_check.exists() {
            Some(full_path)
        } else {
            None
        }
    };

    if let Some(path) = check_path(&steam_path) {
        return Some(path);
    }

    let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(&vdf_path) {
        if let Some(library_paths) = extract_library_paths(&content) {
            for lib_path in library_paths {
                if let Some(path) = check_path(&lib_path) {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn extract_library_paths(content: &str) -> Option<Vec<PathBuf>> {
    let vdf = keyvalues_parser::parse(content).map(keyvalues_parser::Vdf::from).ok()?;
    let mut paths = Vec::new();
    
    if let keyvalues_parser::Value::Obj(root_obj) = &vdf.value {
        for (_key, values) in root_obj.iter() {
            for val in values {
                if let keyvalues_parser::Value::Obj(folder_obj) = val {
                    for (folder_key, folder_values) in folder_obj.iter() {
                        if folder_key.eq_ignore_ascii_case("path") {
                            for path_val in folder_values {
                                if let keyvalues_parser::Value::Str(path_str) = path_val {
                                    let clean_path = path_str.replace("\\\\", "\\");
                                    paths.push(PathBuf::from(clean_path));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    if paths.is_empty() { None } else { Some(paths) }
}

#[cfg(windows)]
pub fn restart_csgo(appid: i32) -> Result<(), LauncherError> {
    close_csgo_if_running()?;

    let steam_dir =
        steam_install_dir().ok_or_else(|| LauncherError::Registry("failed to find Steam install path".to_string()))?;
    let steam = steam_dir.join("steam.exe");
    
    let protocol_string = match appid {
        730 => "steam://launch/730/option2".to_string(),
        _ => format!("steam://launch/{}/dialog", appid),
    };

    Command::new(&steam)
        .args([&protocol_string, "-steam", "-insecure", "-novid"])
        .current_dir(&steam_dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| LauncherError::System(format!("failed to launch {}: {error}", steam.display())))
}

#[cfg(not(windows))]
pub fn restart_csgo(_appid: i32) -> Result<(), LauncherError> {
    Ok(())
}

#[cfg(windows)]
pub fn close_csgo_if_running() -> Result<(), LauncherError> {
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

#[cfg(not(windows))]
pub fn close_csgo_if_running() -> Result<(), LauncherError> {
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