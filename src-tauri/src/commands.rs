//! Tauri commands: game data loading (with cache), settings, craft lists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
        Some(root) => (Some(Install { root: root.clone(), library: None }), true),
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
    let stale = |cache: GameData, warning: String| LoadResult {
        data: cache,
        freshness: Freshness::Stale,
        warning: Some(warning),
        icon_dir: icon_dir.clone(),
    };

    let Some(install) = install_for(&settings, game).0 else {
        return match cache {
            Some(c) => Ok(stale(c, "game-not-found".into())),
            None => Err("game-not-found".into()),
        };
    };

    let fingerprint = extract::fingerprint(&install.root).ok();
    if let (Some(c), Some(fp)) = (&cache, &fingerprint) {
        if &c.fingerprint == fp {
            let data = cache.unwrap();
            return Ok(LoadResult { data, freshness: Freshness::Cached, warning: None, icon_dir });
        }
    }

    match extract::extract(game, &install.root) {
        Ok(extracted) => {
            let mut data = extracted.data;
            if let Some(err) = extracted.icons_error {
                eprintln!("icons of {}: {err}", game.as_str());
            }
            if let Some(set) = extracted.icons {
                // Sheets are named by fingerprint: an open webview never
                // shows a stale image from its cache after a game update.
                let prefix = format!("{}-icons-{:x}", game.as_str(), hash(&data.fingerprint));
                remove_old_sheets(&icon_dir, game);
                match set.write_sheets(&icon_dir, &prefix) {
                    Ok(names) => data.icons.sheets = names,
                    Err(e) => {
                        eprintln!("writing icons of {}: {e}", game.as_str());
                        data.icons = Default::default();
                    }
                }
            }
            write_json(&cache_path(app, game)?, &data)?;
            Ok(LoadResult { data, freshness: Freshness::Extracted, warning: None, icon_dir })
        }
        Err(e) => {
            let code = match &e {
                ExtractError::Outdated(_) => "game-updated",
                _ => "extract-failed",
            };
            eprintln!("extraction of {} failed: {e}", game.as_str());
            match cache {
                Some(c) => Ok(stale(c, code.into())),
                None => Err(format!("{code}: {e}")),
            }
        }
    }
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn remove_old_sheets(dir: &Path, game: GameId) {
    let prefix = format!("{}-icons-", game.as_str());
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with(&prefix) {
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
            extract::fingerprint(&p).map_err(|_| "not-a-game-folder".to_string())?;
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
