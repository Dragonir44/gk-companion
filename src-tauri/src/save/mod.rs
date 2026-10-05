//! Reads the player's progress (unlocked techs and recipes) from saves.
//!
//! gk2 saves are Odin Serializer binaries; the progress lives in
//! `knowledgeSystem` as string lists. gk1 saves use the game's own
//! serialization (see `gk1`). Both yield the same list names.

pub mod gk1;
pub mod odin;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::games::{self, Install};
use crate::model::GameId;
use odin::{Entry, Reader};

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("save ends early at {0} (being written?)")]
    Truncated(usize),
    #[error("invalid save: {0}")]
    Format(String),
    #[error("reading saves of this game is not supported yet")]
    Unsupported,
}

/// `knowledgeSystem` lists sent to the frontend.
const KEPT_LISTS: &[&str] = &[
    "unlockedTechs",
    "revealedTechs",
    "hiddenTechs",
    "unlockedCrafts",
    "unlockedBuildings",
    "lockedBuildings",
    "blackListCrafts",
    // Alchemy: formulas researched, and mixes already made in game.
    "unlockedAlchemyFormulas",
    "knownMixCrafts",
];
const KNOWLEDGE: &str = "knowledgeSystem";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSlot {
    /// File stem, e.g. `Steam_1`.
    pub id: String,
    pub path: PathBuf,
    /// Modification time, ms since epoch: changes on every save.
    pub modified: u64,
    /// The game's `.info` summary (day, date...), when readable.
    pub info: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub slot: String,
    pub modified: u64,
    pub lists: BTreeMap<String, Vec<String>>,
}

pub fn supported(game: GameId) -> bool {
    matches!(game, GameId::Gk1 | GameId::Gk2)
}

/// Company and product names from the data folder's `app.info`: they name
/// the save folder (`LocalLow/<company>/<product>`).
fn app_info(root: &Path) -> Option<(String, String)> {
    let data = crate::extract::data_dir(root).ok()?;
    let text = std::fs::read_to_string(data.join("app.info")).ok()?;
    let mut lines = text.lines().map(str::trim);
    Some((lines.next()?.to_string(), lines.next()?.to_string()))
}

/// Folders that may hold the game's saves (Unity's persistentDataPath):
/// native Linux, macOS and Windows, then Proton prefixes in every library.
pub fn save_dirs(game: GameId, install: &Install) -> Vec<PathBuf> {
    let Some((company, product)) = app_info(&install.root) else { return vec![] };
    let local_low = |base: PathBuf| base.join("AppData/LocalLow").join(&company).join(&product);
    let mut dirs = Vec::new();
    if let Some(config) = dirs::config_dir() {
        dirs.push(config.join("unity3d").join(&company).join(&product));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("Library/Application Support").join(&company).join(&product));
        dirs.push(local_low(home));
    }
    let mut libs: Vec<PathBuf> = install.library.iter().cloned().collect();
    libs.extend(games::libraries());
    for lib in libs {
        let prefix = lib.join(format!("steamapps/compatdata/{}/pfx/drive_c/users/steamuser", game.steam_app_id()));
        dirs.push(local_low(prefix));
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| d.is_dir() && seen.insert(d.canonicalize().unwrap_or_else(|_| d.clone())));
    dirs
}

fn modified_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Save slots, most recently written first. Backup copies are skipped.
pub fn list_slots(dirs: &[PathBuf]) -> Vec<SaveSlot> {
    let mut slots = Vec::new();
    for dir in dirs {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = e.path();
            let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
            if path.extension().is_none_or(|x| x != "dat") || stem.contains("_backup_") {
                continue;
            }
            let info = std::fs::read(path.with_extension("info")).ok().and_then(|b| serde_json::from_slice(&b).ok());
            slots.push(SaveSlot { id: stem, modified: modified_ms(&path), path, info });
        }
    }
    slots.sort_by_key(|s| std::cmp::Reverse(s.modified));
    slots
}

pub fn read_progress(game: GameId, slot: &SaveSlot) -> Result<Progress, SaveError> {
    if !supported(game) {
        return Err(SaveError::Unsupported);
    }
    let bytes = std::fs::read(&slot.path)?;
    let lists = match game {
        GameId::Gk1 => gk1::progress_lists(&bytes)?,
        GameId::Gk2 => knowledge_lists(&bytes)?,
    };
    Ok(Progress { slot: slot.id.clone(), modified: slot.modified, lists })
}

/// String lists directly under `knowledgeSystem/<field>`.
fn knowledge_lists(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>, SaveError> {
    // Node names from the root; arrays push None.
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut lists: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen_knowledge = false;
    let mut r = Reader::new(bytes);
    loop {
        match r.next_entry()? {
            Entry::Start(name, _) => stack.push(name),
            Entry::StartArray => stack.push(None),
            Entry::End | Entry::EndArray => {
                if stack.pop().flatten().as_deref() == Some(KNOWLEDGE) {
                    // Everything needed has been read.
                    break;
                }
            }
            Entry::Str(None, value) => {
                if let [.., Some(k), Some(field), None] = stack.as_slice() {
                    if k == KNOWLEDGE && KEPT_LISTS.contains(&field.as_str()) {
                        seen_knowledge = true;
                        lists.entry(field.clone()).or_default().push(value);
                    }
                }
            }
            Entry::Str(Some(_), _) | Entry::Other(_) => {}
            Entry::EndOfStream => break,
        }
    }
    if !seen_knowledge {
        return Err(SaveError::Format("no knowledgeSystem in save".into()));
    }
    Ok(lists)
}
