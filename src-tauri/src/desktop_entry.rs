//! Linux: makes window decorations show the app icon on Wayland.
//!
//! Wayland compositors (KWin...) take a window's icon from the `.desktop`
//! file named after its app id, which GTK sets to the binary name
//! (`gk-companion`). Installers name that file their own way (Gear Lever:
//! `gk_companion.desktop`, deb/rpm: `GK Companion.desktop`), so no match is
//! found. At startup, if no `<app id>.desktop` exists, a hidden one
//! (`NoDisplay`, never listed in menus) is written with the icon, and kept
//! up to date when the executable moves.

// Only called by release builds (dev builds run from target/).
#![cfg_attr(debug_assertions, allow(dead_code))]

use std::path::{Path, PathBuf};

/// Marks entries written by this module, the only ones it may rewrite.
const MARKER: &str = "X-GKCompanion-Generated=true";
const ICON_PNG: &[u8] = include_bytes!("../icons/128x128@2x.png");
const ICON_SIZE: &str = "256x256";

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
}

fn data_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_home().into_iter().chain(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from)).collect()
}

/// Where the app is launched from: the AppImage itself, not its mount.
fn launcher() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(|| std::env::current_exe().ok())
}

fn entry(app_id: &str, exec: &Path) -> String {
    // Exec arguments are double-quoted, with `"`, `` ` ``, `$` and `\` escaped.
    let exec = exec.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$");
    format!(
        "[Desktop Entry]\nType=Application\nName=GK Companion\nExec=\"{exec}\"\nIcon={app_id}\n\
         StartupWMClass={app_id}\nNoDisplay=true\nTerminal=false\n{MARKER}\n"
    )
}

pub fn ensure() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let app_id = exe.file_stem().ok_or("no executable name")?.to_string_lossy().into_owned();
    let file_name = format!("{app_id}.desktop");

    let existing = data_dirs().into_iter().map(|d| d.join("applications").join(&file_name)).find(|p| p.is_file());
    if let Some(path) = &existing {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        if !content.contains(MARKER) {
            return Ok(()); // installed by a package: it already matches
        }
    }

    let home = data_home().ok_or("no data dir")?;
    let target = home.join("applications").join(&file_name);
    let wanted = entry(&app_id, &launcher().ok_or("no launcher path")?);
    if std::fs::read_to_string(&target).ok().as_deref() != Some(wanted.as_str()) {
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&target, wanted).map_err(|e| e.to_string())?;
    }

    let icon = home.join("icons/hicolor").join(ICON_SIZE).join("apps").join(format!("{app_id}.png"));
    if std::fs::read(&icon).ok().as_deref() != Some(ICON_PNG) {
        std::fs::create_dir_all(icon.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&icon, ICON_PNG).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_hidden_marked_and_quotes_exec() {
        let e = entry("gk-companion", Path::new("/home/a b/Apps/gk$1.AppImage"));
        assert!(e.contains("Exec=\"/home/a b/Apps/gk\\$1.AppImage\"\n"));
        assert!(e.contains("NoDisplay=true"));
        assert!(e.contains("Icon=gk-companion\n"));
        assert!(e.contains(MARKER));
    }

    #[test]
    fn writes_updates_and_respects_package_entries() {
        let tmp = std::env::temp_dir().join(format!("gkc-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("XDG_DATA_HOME", &tmp);
        std::env::set_var("XDG_DATA_DIRS", tmp.join("none"));
        let app_id = std::env::current_exe().unwrap().file_stem().unwrap().to_string_lossy().into_owned();
        let desktop = tmp.join("applications").join(format!("{app_id}.desktop"));

        std::env::set_var("APPIMAGE", "/apps/one.AppImage");
        ensure().unwrap();
        assert!(std::fs::read_to_string(&desktop).unwrap().contains("Exec=\"/apps/one.AppImage\""));
        assert!(tmp.join(format!("icons/hicolor/256x256/apps/{app_id}.png")).is_file());

        // The AppImage moved: our own entry follows it.
        std::env::set_var("APPIMAGE", "/apps/two.AppImage");
        ensure().unwrap();
        assert!(std::fs::read_to_string(&desktop).unwrap().contains("/apps/two.AppImage"));

        // An entry we didn't write is left alone.
        std::fs::write(&desktop, "[Desktop Entry]\nName=pkg\n").unwrap();
        ensure().unwrap();
        assert_eq!(std::fs::read_to_string(&desktop).unwrap(), "[Desktop Entry]\nName=pkg\n");

        std::env::remove_var("APPIMAGE");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
