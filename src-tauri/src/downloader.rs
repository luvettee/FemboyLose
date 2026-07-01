use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};
use crate::error::LauncherError;
use crate::steam;
use crate::theme;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/luvettee/FemboyLose/releases";
const GITHUB_RELEASE_BY_TAG_URL: &str =
    "https://api.github.com/repos/luvettee/FemboyLose/releases/tags";
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
    pub published_at: Option<String>,
    pub updated_at: String,
    pub html_url: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct LauncherGitMetadata {
    pub releases: Vec<LauncherVersion>,
    pub nightlies: Vec<LauncherVersion>,
}

#[derive(Debug, Serialize)]
pub struct LauncherVersion {
    pub tag: String,
    pub name: String,
    pub changelog: String,
    pub updated_at: String,
    pub url: String,
    pub assets: Vec<LauncherAsset>,
}

#[derive(Debug, Serialize)]
pub struct LauncherAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

pub fn github_client() -> Result<reqwest::Client, LauncherError> {
    reqwest::Client::builder()
        .user_agent("NeverloseTauriOfficial")
        .build()
        .map_err(|error| LauncherError::Reqwest(format!("failed to create GitHub client: {error}")))
}

pub async fn load_git_metadata() -> Result<LauncherGitMetadata, LauncherError> {
    let client = github_client()?;
    let releases = client
        .get(GITHUB_RELEASES_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<GithubRelease>>()
        .await?;

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

pub fn version_install_dir(tag: &str) -> Result<PathBuf, LauncherError> {
    let appdata = std::env::var("APPDATA")
        .map_err(|error| LauncherError::System(format!("APPDATA is not available: {error}")))?;
    Ok(Path::new(&appdata)
        .join("neverlose")
        .join("bin")
        .join(theme::sanitize_path_segment(tag)))
}

pub async fn download_and_launch_version(
    tag: String,
    config_id: Option<i32>,
    appid: i32,
    auto_launch: bool,
) -> Result<(), LauncherError> {
    if tag.trim().is_empty() || tag == "Unavailable" {
        return Err(LauncherError::Validation("no release version is selected".to_string()));
    }

    let client = github_client()?;
    let release = client
        .get(format!("{GITHUB_RELEASE_BY_TAG_URL}/{tag}"))
        .send()
        .await?
        .error_for_status()?
        .json::<GithubRelease>()
        .await?;

    let install_dir = version_install_dir(&tag)?;
    tokio::fs::create_dir_all(&install_dir)
        .await
        .map_err(|error| LauncherError::Io(format!("failed to create {}: {error}", install_dir.display())))?;

    #[cfg(windows)]
    let _ = kill_background_processes();

    // Clean up any left-over .old files from previous locked-file workarounds
    if let Ok(mut entries) = tokio::fs::read_dir(&install_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "old" {
                        let _ = tokio::fs::remove_file(&path).await;
                    }
                }
            }
        }
    }

    // Download and parse SHA256SUMS.txt if it exists as an asset
    let mut checksums = HashMap::new();
    if let Some(sums_asset) = release.assets.iter().find(|asset| asset.name.eq_ignore_ascii_case("SHA256SUMS.txt")) {
        if let Ok(response) = client.get(&sums_asset.browser_download_url).send().await {
            if let Ok(text) = response.text().await {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let hash = parts[0].to_string().to_lowercase();
                        let filename = parts[1].trim_start_matches('*').to_string();
                        checksums.insert(filename, hash);
                    }
                }
            }
        }
    }

    download_asset(&client, &release, "neverlose.dll", &install_dir, &checksums).await?;
    download_asset(&client, &release, "neverlose-server.exe", &install_dir, &checksums).await?;
    download_asset(&client, &release, "injector.exe", &install_dir, &checksums).await?;
    // download_asset(&client, &release, "old_injector.exe", &install_dir, &checksums).await?; // add third option to injector and nuke this line pls(put injector from 1.0.6 for time being pls)
    let game_folder_name = if appid == 730 {
        "Counter-Strike Global Offensive"
    } else {
        "csgo legacy"
    };

    let game_dir = steam::find_game_install_path(game_folder_name);
    let cloud_dir = if let Some(ref dir) = game_dir {
        dir.join("nl_cloud")
    } else {
        theme::launcher_cloud_dir()?
    };

    tokio::fs::create_dir_all(&cloud_dir)
        .await
        .map_err(|error| LauncherError::Io(format!("failed to create {}: {error}", cloud_dir.display())))?;

    spawn_server_hidden(
        &install_dir.join("neverlose-server.exe"),
        &install_dir,
        &cloud_dir,
        config_id,
    )?;

    if auto_launch {
        if is_legacy_version(&tag) {
            steam::restart_csgo(appid)?;
            spawn_hidden(&install_dir.join("injector.exe"), &install_dir)?;
        } else {
            let headless_choice = if appid == 730 { 2 } else { 1 };
            spawn_injector_headless(&install_dir.join("injector.exe"), &install_dir, headless_choice, game_dir)?;
        }
    } else {
        spawn_hidden(&install_dir.join("old_injector.exe"), &install_dir)?; // workaround im lazy as fuk to download visaul studio (its the old injector so make sure to bundle it ig)
    }
    Ok(())
}

async fn calculate_file_sha256(path: &Path) -> Result<String, LauncherError> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| LauncherError::Io(format!("failed to open file for hashing: {e}")))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .await
            .map_err(|e| LauncherError::Io(format!("failed to read file for hashing: {e}")))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn download_asset(
    client: &reqwest::Client,
    release: &GithubRelease,
    asset_name: &str,
    install_dir: &Path,
    checksums: &HashMap<String, String>,
) -> Result<(), LauncherError> {
    let target = install_dir.join(asset_name);

    if target.exists() {
        if let Some(expected_hash) = checksums.get(asset_name) {
            if let Ok(actual_hash) = calculate_file_sha256(&target).await {
                if actual_hash.eq_ignore_ascii_case(expected_hash) {
                    return Ok(());
                }
            }
        }
    }

    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(asset_name))
        .ok_or_else(|| LauncherError::Validation(format!("release {} is missing {asset_name}", release.tag_name)))?;
    
    let temp = install_dir.join(format!("{asset_name}.download"));
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    tokio::fs::write(&temp, bytes)
        .await
        .map_err(|error| LauncherError::Io(format!("failed to write {}: {error}", temp.display())))?;

    if let Some(expected_hash) = checksums.get(asset_name) {
        let actual_hash = calculate_file_sha256(&temp).await?;
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(LauncherError::Validation(format!(
                "checksum validation failed for downloaded {asset_name} (expected {expected_hash}, got {actual_hash})"
            )));
        }
    }

    if target.exists() {
        if let Err(_) = tokio::fs::remove_file(&target).await {
            let backup_name = format!(
                "{}.{}.old",
                asset_name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            );
            let backup_path = install_dir.join(backup_name);
            tokio::fs::rename(&target, &backup_path)
                .await
                .map_err(|error| LauncherError::Io(format!("failed to rename locked file {} to backup: {error}", target.display())))?;
        }
    }

    tokio::fs::rename(&temp, &target)
        .await
        .map_err(|error| LauncherError::Io(format!("failed to move {} into place: {error}", target.display())))?;

    Ok(())
}

pub fn kill_background_processes() -> Result<(), LauncherError> {
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/im", "injector.exe", "/f"])
            .spawn()
            .map(|_| ())
            .map_err(|error| LauncherError::System(format!("failed to kill injector: {error}")))?;

        Command::new("taskkill")
            .args(["/im", "neverlose-server.exe", "/f"])
            .spawn()
            .map(|_| ())
            .map_err(|error| LauncherError::System(format!("failed to kill neverlose server: {error}")))?;
    }
    Ok(())
}

fn spawn_hidden(exe: &Path, working_dir: &Path) -> Result<(), LauncherError> {
    let mut command = Command::new(exe);
    command.current_dir(working_dir);

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| LauncherError::System(format!("failed to launch {}: {error}", exe.display())))
}

fn spawn_injector_headless(
    exe: &Path,
    working_dir: &Path,
    choice: i32,
    game_dir: Option<PathBuf>,
) -> Result<(), LauncherError> {
    let mut command = Command::new(exe);
    command.current_dir(working_dir);
    command.env("INJECTOR_HEADLESS", choice.to_string());
    if let Some(dir) = game_dir {
        command.env("INJECTOR_GAME_DIR", dir);
    }

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| LauncherError::System(format!("failed to launch injector: {error}")))
}

fn spawn_server_hidden(
    exe: &Path,
    working_dir: &Path,
    cloud_dir: &Path,
    config_id: Option<i32>,
) -> Result<(), LauncherError> {
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
        .map_err(|error| LauncherError::System(format!("failed to launch {}: {error}", exe.display())))
}

pub fn is_legacy_version(tag: &str) -> bool {
    let clean_tag = tag.trim_start_matches('v');
    let parts: Vec<&str> = clean_tag.split('.').collect();
    if parts.is_empty() {
        return false;
    }
    if let Ok(major) = parts[0].parse::<i32>() {
        if major < 1 {
            return true;
        }
        if major == 1 && parts.len() > 1 {
            if let Ok(minor) = parts[1].parse::<i32>() {
                if minor < 1 {
                    return true;
                }
            }
        }
    }
    false
}
