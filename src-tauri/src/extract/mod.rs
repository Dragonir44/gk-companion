//! Extracts recipes, techs and texts from an installed game.

mod expr;
mod gk1;
mod gk2;
mod names;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::model::{Entity, GameData, GameId, Recipe, Tech, MODEL_VERSION};
use crate::unity::serialized::SerializedFile;
use crate::unity::typetree::Schema;
use crate::unity::UnityError;
use names::Names;

const GK1_SCHEMA: &str = include_str!("../../schemas/gk1.json");
const GK2_SCHEMA: &str = include_str!("../../schemas/gk2.json");

/// Localization objects are all the same class; the schema stores it once.
const LNG_CLASS: &str = "lng_en";
const MIN_DATA_OBJECT_SIZE: u32 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Unity(#[from] UnityError),
    #[error("data not found in game files: {0}")]
    Missing(String),
    /// The game was updated in a way this app version can't read yet.
    #[error("game data layout changed ({0}); an app update is needed")]
    Outdated(String),
}

impl From<std::io::Error> for ExtractError {
    fn from(e: std::io::Error) -> Self {
        ExtractError::Unity(e.into())
    }
}

/// What a game-specific normalizer produces.
pub struct Normalized {
    pub items: Vec<Entity>,
    pub objects: Vec<Entity>,
    pub groups: BTreeMap<String, Vec<String>>,
    pub recipes: Vec<Recipe>,
    pub techs: Vec<Tech>,
}

fn balance_object(game: GameId) -> &'static str {
    match game {
        GameId::Gk1 => "game_data",
        GameId::Gk2 => "GameBalance",
    }
}

fn data_dir(root: &Path) -> Result<PathBuf, ExtractError> {
    let entries = std::fs::read_dir(root)?;
    for e in entries.flatten() {
        let name = e.file_name();
        if name.to_string_lossy().ends_with("_Data") && e.path().join("resources.assets").is_file() {
            return Ok(e.path());
        }
    }
    // macOS bundles keep data under Contents/Resources/Data.
    let mac = root.join("Contents/Resources/Data");
    if mac.join("resources.assets").is_file() {
        return Ok(mac);
    }
    Err(ExtractError::Missing(format!("no *_Data folder in {}", root.display())))
}

/// Changes whenever the files the extraction reads change.
pub fn fingerprint(root: &Path) -> Result<String, ExtractError> {
    let data = data_dir(root)?;
    let mut parts = vec![format!("m{MODEL_VERSION}")];
    for f in ["resources.assets", "Managed/Assembly-CSharp.dll"] {
        let p = data.join(f);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs());
        parts.push(format!("{}:{}", meta.len(), mtime.unwrap_or(0)));
    }
    Ok(parts.join("-"))
}

pub fn extract(game: GameId, root: &Path) -> Result<GameData, ExtractError> {
    let schema = Schema::parse(match game {
        GameId::Gk1 => GK1_SCHEMA,
        GameId::Gk2 => GK2_SCHEMA,
    })?;
    let data = data_dir(root)?;
    let file = SerializedFile::open(&data.join("resources.assets"))?;
    let balance_name = balance_object(game);

    let found = file.find_monobehaviours(|n| n == balance_name || n.starts_with("lng_"), MIN_DATA_OBJECT_SIZE)?;

    let decode = |class: &str, name: &str, obj| -> Result<Value, ExtractError> {
        let expected = &schema.classes.get(class).ok_or_else(|| ExtractError::Missing(class.into()))?.type_hash;
        let actual = file.type_hash_hex(obj);
        if *expected != actual {
            return Err(ExtractError::Outdated(format!("{name}: type hash {actual}, expected {expected}")));
        }
        let raw = file.read_object(obj)?;
        schema.decode(class, &raw).map_err(|e| match e {
            UnityError::LayoutMismatch(m) => ExtractError::Outdated(m),
            e => e.into(),
        })
    };

    let (_, balance_obj) = found
        .iter()
        .find(|(n, _)| n == balance_name)
        .ok_or_else(|| ExtractError::Missing(balance_name.into()))?;
    let balance = decode(balance_name, balance_name, balance_obj)?;

    let mut locales: BTreeMap<String, HashMap<String, String>> = BTreeMap::new();
    let mut aliases = HashMap::new();
    for (name, obj) in found.iter().filter(|(n, _)| n.starts_with("lng_")) {
        let v = decode(LNG_CLASS, name, obj)?;
        let lang = name.trim_start_matches("lng_").to_string();
        let (ids, txts) = (strings(&v, &["txt_ids", "txtIds"]), strings(&v, &["txts"]));
        if lang == "en" {
            let (a1, a2) = (strings(&v, &["aliases_1", "aliases1"]), strings(&v, &["aliases_2", "aliases2"]));
            aliases = a1.into_iter().zip(a2).collect();
        }
        locales.insert(lang, ids.into_iter().zip(txts).collect());
    }
    let en = locales.get("en").ok_or_else(|| ExtractError::Missing("lng_en".into()))?;
    let mut names = Names::new(en.keys().cloned(), aliases);

    let mut n = match game {
        GameId::Gk1 => gk1::normalize(&balance, &mut names),
        GameId::Gk2 => gk2::normalize(&balance, &mut names),
    };
    hide_test_content(&mut n.recipes);
    for r in &mut n.recipes {
        r.name = names.exact(&r.id);
    }
    name_referenced_items(&mut n, &mut names);

    Ok(GameData {
        model_version: MODEL_VERSION,
        game,
        fingerprint: fingerprint(root)?,
        unity_version: file.unity_version.clone(),
        items: n.items,
        objects: n.objects,
        groups: n.groups,
        recipes: n.recipes,
        techs: n.techs,
        locales: names.filter_locales(locales),
    })
}

/// Developer test ids contain a `test` segment (`test_flitch`, `slava_test_builder`).
fn is_test(id: &str) -> bool {
    id.split(['_', ':']).any(|p| p.eq_ignore_ascii_case("test"))
}

/// The games ship debug recipes and stations; they would otherwise be picked
/// as producers. Hidden, not removed: still reachable with "show hidden".
fn hide_test_content(recipes: &mut [Recipe]) {
    for r in recipes {
        let stacks = r.inputs.iter().chain(&r.outputs).map(|s| s.item.as_str());
        if is_test(&r.id) || r.stations.iter().any(|s| is_test(s)) || stacks.into_iter().any(is_test) {
            r.hidden = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_test;

    #[test]
    fn detects_test_ids() {
        assert!(is_test("test_flitch"));
        assert!(is_test("slava_test_builder:p:garden_empty"));
        assert!(is_test("cow_test_p"));
        assert!(!is_test("contest_board"));
        assert!(!is_test("testament"));
    }
}

// --- JSON helpers shared by the normalizers -------------------------------

fn strings(v: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_array))
        .map(|a| a.iter().map(|s| s.as_str().unwrap_or_default().to_string()).collect())
        .unwrap_or_default()
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or_default()
}

/// Unity serializes bools as UInt8.
fn flag(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(n) => n.as_u64().unwrap_or(0) != 0,
        None => false,
    }
}

fn entity(names: &mut Names, id: &str) -> Entity {
    let name = names.name(id);
    let desc = name.as_deref().and_then(|k| names.desc(k));
    Entity { id: id.to_string(), name, desc, heavy: false }
}

/// Recipes often use an item without its quality suffix (`meal:burger`)
/// while the item table only lists the variants (`meal:burger:1`..`3`):
/// give every referenced id an entity so it gets a name.
fn name_referenced_items(n: &mut Normalized, names: &mut Names) {
    let mut known: std::collections::HashSet<String> = n.items.iter().map(|e| e.id.clone()).collect();
    known.extend(n.groups.keys().cloned());
    for r in &n.recipes {
        for s in r.inputs.iter().chain(&r.outputs) {
            if known.insert(s.item.clone()) {
                n.items.push(entity(names, &s.item));
            }
        }
    }
}

/// Item sizes above 1 are carried overhead.
const HEAVY_ITEM_SIZE: f64 = 2.0;

fn item_entity(names: &mut Names, item: &Value, size_field: &str) -> Entity {
    Entity { heavy: num(item, size_field) >= HEAVY_ITEM_SIZE, ..entity(names, text(item, "id")) }
}

/// Objects referenced by recipes (stations, built objects), in a stable order.
fn referenced_objects(recipes: &[Recipe], names: &mut Names) -> Vec<Entity> {
    let mut ids: Vec<&str> = recipes
        .iter()
        .flat_map(|r| r.stations.iter().chain(r.builds.iter()))
        .map(String::as_str)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().map(|id| entity(names, id)).collect()
}
