//! Tauri commands: game data loading (with cache), settings, craft lists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::extract::{self, ExtractError};
use crate::games::{self, Install};
use crate::model::{GameData, GameId, MODEL_VERSION};
use crate::save::{self, Progress, SaveSlot};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Manually chosen install folders, overriding Steam detection.
    pub game_paths: BTreeMap<GameId, PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub game: GameId,
    pub install: Option<Install>,
    pub manual: bool,
    pub cached: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    /// Just extracted from the game files.
    Extracted,
    /// Cache matches the current game files.
    Cached,
    /// Cache from older game files: the game is missing or unreadable.
    Stale,
    /// Cache from older game files, shown while re-extracting.
    Refreshing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadResult {
    pub data: GameData,
    pub freshness: Freshness,
    pub warning: Option<String>,
    /// Directory holding the icon sheets named in `data.icons.sheets`.
    pub icon_dir: PathBuf,
}

type CmdResult<T> = Result<T, String>;

fn dir(app: &AppHandle, config: bool) -> CmdResult<PathBuf> {
    let p = if config { app.path().app_config_dir() } else { app.path().app_data_dir() };
    let p = p.map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Writes through a temp file so a crash never leaves a truncated file.
fn write_json<T: Serialize>(path: &Path, value: &T) -> CmdResult<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn settings_path(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(dir(app, true)?.join("settings.json"))
}

fn load_settings(app: &AppHandle) -> CmdResult<Settings> {
    Ok(read_json(&settings_path(app)?).unwrap_or_default())
}

fn cache_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    let d = dir(app, false)?.join("cache");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn cache_path(app: &AppHandle, game: GameId) -> CmdResult<PathBuf> {
    Ok(cache_dir(app)?.join(format!("{}.json", game.as_str())))
}

/// A cache is usable when its model is current and its icon sheets exist.
fn read_cache(app: &AppHandle, game: GameId) -> Option<GameData> {
    let bytes = std::fs::read(cache_path(app, game).ok()?).ok()?;
    // Unreadable caches are re-extracted, which can take seconds: say why.
    let data: GameData = serde_json::from_slice(&bytes)
        .map_err(|e| eprintln!("cache of {} unreadable, re-extracting: {e}", game.as_str()))
        .ok()?;
    let dir = cache_dir(app).ok()?;
    let sheets_ok = data.icons.sheets.iter().all(|s| dir.join(s).is_file());
    (data.model_version == MODEL_VERSION && sheets_ok).then_some(data)
}

fn install_for(settings: &Settings, game: GameId) -> (Option<Install>, bool) {
    match settings.game_paths.get(&game) {
        Some(root) => (Some(games::manual_install(root.clone(), game)), true),
        None => (games::find_steam_install(game), false),
    }
}

#[tauri::command]
pub fn game_statuses(app: AppHandle) -> CmdResult<Vec<GameStatus>> {
    let settings = load_settings(&app)?;
    Ok(GameId::ALL
        .into_iter()
        .map(|game| {
            let (install, manual) = install_for(&settings, game);
            let cached = cache_path(&app, game).map(|p| p.is_file()).unwrap_or(false);
            GameStatus { game, install, manual, cached }
        })
        .collect())
}

/// Loads a game's data: from cache when the game files are unchanged,
/// otherwise re-extracted. Falls back to the cache when extraction fails.
#[tauri::command]
pub async fn load_game(app: AppHandle, game: GameId) -> CmdResult<LoadResult> {
    tauri::async_runtime::spawn_blocking(move || load_game_blocking(&app, game))
        .await
        .map_err(|e| e.to_string())?
}

fn load_game_blocking(app: &AppHandle, game: GameId) -> CmdResult<LoadResult> {
    let settings = load_settings(app)?;
    let cache = read_cache(app, game);
    let icon_dir = cache_dir(app)?;
    let result = |data: GameData, freshness: Freshness, warning: Option<&str>| LoadResult {
        data,
        freshness,
        warning: warning.map(str::to_string),
        icon_dir: icon_dir.clone(),
    };

    let Some(install) = install_for(&settings, game).0 else {
        return match cache {
            Some(c) => Ok(result(c, Freshness::Stale, Some("game-not-found"))),
            None => Err("game-not-found".into()),
        };
    };

    let fingerprint = extract::fingerprint(&install.root, install.build_id.as_deref()).ok();
    match cache {
        Some(c) if fingerprint.as_ref() == Some(&c.fingerprint) => Ok(result(c, Freshness::Cached, None)),
        // The game changed: show the previous data now, refresh behind it.
        Some(c) => {
            refresh_in_background(app.clone(), game, install);
            Ok(result(c, Freshness::Refreshing, None))
        }
        None => match extract_and_cache(app, game, &install) {
            Ok(data) => Ok(result(data, Freshness::Extracted, None)),
            Err((code, e)) => Err(format!("{code}: {e}")),
        },
    }
}

/// Extracts a game and writes its cache (data + icon sheets).
/// Errors carry a code for the frontend: `game-updated` or `extract-failed`.
fn extract_and_cache(app: &AppHandle, game: GameId, install: &Install) -> Result<GameData, (&'static str, String)> {
    let failed = |e: String| ("extract-failed", e);
    let icon_dir = cache_dir(app).map_err(failed)?;
    let extracted = extract::extract(game, &install.root, install.build_id.as_deref()).map_err(|e| {
        eprintln!("extraction of {} failed: {e}", game.as_str());
        let code = if matches!(e, ExtractError::Outdated(_)) { "game-updated" } else { "extract-failed" };
        (code, e.to_string())
    })?;
    let mut data = extracted.data;
    if let Some(err) = extracted.icons_error {
        eprintln!("icons of {}: {err}", game.as_str());
    }
    if let Some(set) = extracted.icons {
        // Sheets are named by fingerprint: an open webview never shows a
        // stale image from its cache after a game update.
        let prefix = format!("{}-icons-{:x}", game.as_str(), hash(&data.fingerprint));
        match set.write_sheets(&icon_dir, &prefix) {
            Ok(names) => {
                // Only now: the sheets on screen stay valid until the switch.
                remove_old_sheets(&icon_dir, game, &prefix);
                data.icons.sheets = names;
            }
            Err(e) => {
                eprintln!("writing icons of {}: {e}", game.as_str());
                data.icons = Default::default();
            }
        }
    }
    write_json(&cache_path(app, game).map_err(failed)?, &data).map_err(failed)?;
    Ok(data)
}

/// Games being re-extracted in the background, never twice at once.
static REFRESHING: std::sync::Mutex<Vec<GameId>> = std::sync::Mutex::new(Vec::new());

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshFailed {
    game: GameId,
    code: &'static str,
}

/// Re-extracts a game whose files changed, then emits `game-data-updated`
/// (the frontend reloads it from the fresh cache) or `game-data-failed`.
fn refresh_in_background(app: AppHandle, game: GameId, install: Install) {
    {
        let mut running = REFRESHING.lock().unwrap();
        if running.contains(&game) {
            return;
        }
        running.push(game);
    }
    std::thread::spawn(move || {
        let outcome = extract_and_cache(&app, game, &install);
        REFRESHING.lock().unwrap().retain(|g| *g != game);
        let _ = match outcome {
            Ok(_) => app.emit("game-data-updated", game),
            Err((code, _)) => app.emit("game-data-failed", RefreshFailed { game, code }),
        };
    });
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Removes a game's icon sheets except the current ones (`keep` prefix).
fn remove_old_sheets(dir: &Path, game: GameId, keep: &str) {
    let prefix = format!("{}-icons-", game.as_str());
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && !name.starts_with(keep) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Sets (or clears, with None) the install folder of a game.
#[tauri::command]
pub fn set_game_path(app: AppHandle, game: GameId, path: Option<PathBuf>) -> CmdResult<()> {
    let mut settings = load_settings(&app)?;
    match path {
        Some(p) => {
            extract::fingerprint(&p, None).map_err(|_| "not-a-game-folder".to_string())?;
            settings.game_paths.insert(game, p);
        }
        None => {
            settings.game_paths.remove(&game);
        }
    }
    write_json(&settings_path(&app)?, &settings)
}

fn lists_path(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(dir(app, false)?.join("lists.json"))
}

/// Craft lists are owned by the frontend; the backend only persists them.
#[tauri::command]
pub fn load_lists(app: AppHandle) -> CmdResult<serde_json::Value> {
    Ok(read_json(&lists_path(&app)?).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub fn save_lists(app: AppHandle, lists: serde_json::Value) -> CmdResult<()> {
    write_json(&lists_path(&app)?, &lists)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSlots {
    /// Whether this app can read the game's saves at all.
    pub supported: bool,
    pub slots: Vec<SaveSlot>,
}

fn slots_for(app: &AppHandle, game: GameId) -> CmdResult<Vec<SaveSlot>> {
    let settings = load_settings(app)?;
    let Some(install) = install_for(&settings, game).0 else { return Ok(vec![]) };
    Ok(save::list_slots(&save::save_dirs(game, &install)))
}

/// Save slots, most recent first. Cheap (file metadata only): polled.
#[tauri::command]
pub async fn save_slots(app: AppHandle, game: GameId) -> CmdResult<SaveSlots> {
    tauri::async_runtime::spawn_blocking(move || {
        let supported = save::supported(game);
        let slots = if supported { slots_for(&app, game)? } else { vec![] };
        Ok(SaveSlots { supported, slots })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn read_save(app: AppHandle, game: GameId, slot: String) -> CmdResult<Progress> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = slots_for(&app, game)?.into_iter().find(|s| s.id == slot).ok_or("save-not-found")?;
        save::read_progress(game, &s).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDiagnostics {
    pub game: GameId,
    pub found: bool,
    pub manual: bool,
    pub build_id: Option<String>,
    /// Steam, Proton prefix or not found: where saves are looked for.
    pub save_dirs: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub app_version: String,
    /// e.g. "CachyOS Linux Rolling [64-bit]".
    pub os: String,
    pub arch: &'static str,
    pub games: Vec<GameDiagnostics>,
}

/// Environment summary pasted into bug reports. Paths are left out: they
/// would show user names.
#[tauri::command]
pub fn diagnostics(app: AppHandle) -> CmdResult<Diagnostics> {
    let settings = load_settings(&app)?;
    let games = GameId::ALL
        .into_iter()
        .map(|game| {
            let (install, manual) = install_for(&settings, game);
            GameDiagnostics {
                game,
                found: install.is_some(),
                manual,
                build_id: install.as_ref().and_then(|i| i.build_id.clone()),
                save_dirs: install.as_ref().map_or(0, |i| save::save_dirs(game, i).len()),
            }
        })
        .collect();
    Ok(Diagnostics {
        app_version: app.package_info().version.to_string(),
        os: os_info::get().to_string(),
        arch: std::env::consts::ARCH,
        games,
    })
}
