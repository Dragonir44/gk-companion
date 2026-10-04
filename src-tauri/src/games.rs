//! Locates game installs through Steam libraries.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::GameId;

impl GameId {
    pub fn steam_app_id(self) -> u32 {
        match self {
            GameId::Gk1 => 599140,
            GameId::Gk2 => 4358690,
        }
    }

    pub const ALL: [GameId; 2] = [GameId::Gk1, GameId::Gk2];
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub root: PathBuf,
    /// Steam library holding the game (for Proton prefixes); None if set manually.
    pub library: Option<PathBuf>,
    /// Steam build id of the installed version, when Steam manages it.
    pub build_id: Option<String>,
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".local/share/Steam",
            ".steam/steam",
            ".steam/root",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            "snap/steam/common/.local/share/Steam",
            "Library/Application Support/Steam",
        ] {
            roots.push(home.join(rel));
        }
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(pf) = std::env::var_os(var) {
            roots.push(PathBuf::from(pf).join("Steam"));
        }
    }
    roots.retain(|r| r.join("steamapps").is_dir());
    roots
}

/// Library paths listed in `libraryfolders.vdf` (plus the root itself).
pub fn libraries() -> Vec<PathBuf> {
    let mut libs = Vec::new();
    for root in steam_roots() {
        libs.push(root.clone());
        let vdf = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
        libs.extend(vdf_values(&vdf, "path").into_iter().map(PathBuf::from));
    }
    // Symlinked roots (~/.steam/steam -> ~/.local/share/Steam) list the same libraries.
    let mut seen = std::collections::HashSet::new();
    libs.retain(|l| seen.insert(l.canonicalize().unwrap_or_else(|_| l.clone())));
    libs
}

/// Values of `"key" "value"` pairs in a Valve KeyValues file.
fn vdf_values(src: &str, key: &str) -> Vec<String> {
    let quoted_key = format!("\"{key}\"");
    src.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix(&quoted_key)?.trim();
            let v = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(v.replace("\\\\", "\\"))
        })
        .collect()
}

/// Install root and Steam build id of a game in a library.
fn install_in_library(lib: &Path, game: GameId) -> Option<(PathBuf, Option<String>)> {
    let manifest = lib.join(format!("steamapps/appmanifest_{}.acf", game.steam_app_id()));
    let acf = std::fs::read_to_string(manifest).ok()?;
    let dir = vdf_values(&acf, "installdir").into_iter().next()?;
    let build = vdf_values(&acf, "buildid").into_iter().next().filter(|b| !b.is_empty() && b != "0");
    let root = lib.join("steamapps/common").join(dir);
    root.is_dir().then_some((root, build))
}

pub fn find_steam_install(game: GameId) -> Option<Install> {
    libraries().into_iter().find_map(|lib| {
        install_in_library(&lib, game).map(|(root, build_id)| Install { root, library: Some(lib), build_id })
    })
}

/// A manually chosen folder: still Steam's if it is a library's install dir.
pub fn manual_install(root: PathBuf, game: GameId) -> Install {
    let same = |a: &Path, b: &Path| a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(a, b)| a == b);
    let steam = libraries()
        .into_iter()
        .find_map(|lib| install_in_library(&lib, game).filter(|(r, _)| same(r, &root)).map(|(_, b)| (lib, b)));
    match steam {
        Some((lib, build_id)) => Install { root, library: Some(lib), build_id },
        None => Install { root, library: None, build_id: None },
    }
}

#[cfg(test)]
mod tests {
    use super::vdf_values;

    #[test]
    fn parses_vdf_pairs() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        assert_eq!(vdf_values(vdf, "path"), vec!["/home/u/.local/share/Steam", "D:\\SteamLibrary"]);
        assert_eq!(vdf_values(r#"	"installdir"		"Graveyard Keeper 2""#, "installdir"), vec!["Graveyard Keeper 2"]);
    }
}
