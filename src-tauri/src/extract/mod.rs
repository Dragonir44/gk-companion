//! Extracts recipes, techs and texts from an installed game.

mod expr;
mod gk1;
mod gk2;
pub mod icons;
mod names;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::model::{Entity, GameData, GameId, Recipe, Tech, MODEL_VERSION};
use crate::unity::serialized::{ObjectInfo, SerializedFile};
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
    #[error("icons: {0}")]
    Icons(String),
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
    /// Tree tabs: branch id and the text key of its name.
    pub branches: Vec<(i64, String)>,
    /// Icon sprite per world object id, when the game names one.
    pub object_icons: HashMap<String, String>,
}

/// Extraction result; icons are optional: data stays usable without them.
pub struct Extracted {
    pub data: GameData,
    pub icons: Option<icons::IconSet>,
    pub icons_error: Option<String>,
}

fn balance_object(game: GameId) -> &'static str {
    match game {
        GameId::Gk1 => "game_data",
        GameId::Gk2 => "GameBalance",
    }
}

/// Unity's data folder of a game install. `root` may be:
/// - the install folder holding `<Game>_Data` (Windows, Linux);
/// - the `.app` bundle itself (macOS: `Contents/Resources/Data`);
/// - the install folder holding that `.app` (macOS Steam installs).
pub fn data_dir(root: &Path) -> Result<PathBuf, ExtractError> {
    let is_data = |p: &Path| p.join("resources.assets").is_file();
    let mac = |app: &Path| app.join("Contents/Resources/Data");
    if is_data(&mac(root)) {
        return Ok(mac(root));
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    let name_ends = |p: &Path, suffix: &str| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(suffix));
    if let Some(d) = entries.iter().find(|p| name_ends(p, "_Data") && is_data(p)) {
        return Ok(d.clone());
    }
    if let Some(app) = entries.iter().find(|p| name_ends(p, ".app") && is_data(&mac(p))) {
        return Ok(mac(app));
    }
    Err(ExtractError::Missing(format!("no Unity data folder in {}", root.display())))
}

/// Changes whenever the files the extraction reads change. With a Steam
/// build id, file dates are left out: Steam may touch files (verify,
/// move) without changing the game.
pub fn fingerprint(root: &Path, build_id: Option<&str>) -> Result<String, ExtractError> {
    let data = data_dir(root)?;
    let mut parts = vec![format!("m{MODEL_VERSION}")];
    if let Some(b) = build_id {
        parts.push(format!("b{b}"));
    }
    // The Addressables catalog changes whenever gk2's bundles do.
    for f in ["resources.assets", "Managed/Assembly-CSharp.dll", "StreamingAssets/aa/catalog.bin"] {
        let p = data.join(f);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs());
        match build_id {
            Some(_) => parts.push(meta.len().to_string()),
            None => parts.push(format!("{}:{}", meta.len(), mtime.unwrap_or(0))),
        }
    }
    Ok(parts.join("-"))
}

/// The game's data objects: `GameBalance` and the `lng_*` texts.
struct GameObjects {
    schema: Schema,
    data: PathBuf,
    file: SerializedFile,
    found: Vec<(String, ObjectInfo)>,
}

fn open_game(game: GameId, root: &Path) -> Result<GameObjects, ExtractError> {
    let schema = Schema::parse(match game {
        GameId::Gk1 => GK1_SCHEMA,
        GameId::Gk2 => GK2_SCHEMA,
    })?;
    let data = data_dir(root)?;
    let file = SerializedFile::open(&data.join("resources.assets"))?;
    let balance_name = balance_object(game);
    let found = file.find_monobehaviours(|n| n == balance_name || n.starts_with("lng_"), MIN_DATA_OBJECT_SIZE)?;
    Ok(GameObjects { schema, data, file, found })
}

impl GameObjects {
    /// Decodes an object, checking first that its layout is the schema's.
    fn decode(&self, class: &str, name: &str, obj: &ObjectInfo) -> Result<Value, ExtractError> {
        let expected = &self.schema.classes.get(class).ok_or_else(|| ExtractError::Missing(class.into()))?.type_hash;
        let actual = self.file.type_hash_hex(obj);
        if *expected != actual {
            return Err(ExtractError::Outdated(format!("{name}: type hash {actual}, expected {expected}")));
        }
        let raw = self.file.read_object(obj)?;
        self.schema.decode(class, &raw).map_err(|e| match e {
            UnityError::LayoutMismatch(m) => ExtractError::Outdated(m),
            e => e.into(),
        })
    }

    fn balance(&self, game: GameId) -> Result<Value, ExtractError> {
        let name = balance_object(game);
        let (_, obj) = self.found.iter().find(|(n, _)| n == name).ok_or_else(|| ExtractError::Missing(name.into()))?;
        self.decode(name, name, obj)
    }
}

/// The game's `GameBalance`, decoded but not normalized: for investigating
/// game data (`cargo run --example dump`).
pub fn raw_balance(game: GameId, root: &Path) -> Result<Value, ExtractError> {
    open_game(game, root)?.balance(game)
}

/// A language's raw text table (`lng_<lang>`), for investigating names.
pub fn raw_locale(game: GameId, root: &Path, lang: &str) -> Result<Value, ExtractError> {
    let objects = open_game(game, root)?;
    let name = format!("lng_{lang}");
    let (_, obj) = objects.found.iter().find(|(n, _)| *n == name).ok_or_else(|| ExtractError::Missing(name.clone()))?;
    objects.decode(LNG_CLASS, &name, obj)
}

pub fn extract(game: GameId, root: &Path, build_id: Option<&str>) -> Result<Extracted, ExtractError> {
    let objects = open_game(game, root)?;
    let GameObjects { schema, data, file, found } = &objects;
    let decode = |class: &str, name: &str, obj: &ObjectInfo| objects.decode(class, name, obj);
    let balance = objects.balance(game)?;

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
    let branches = n
        .branches
        .iter()
        .map(|(id, key)| crate::model::Branch { id: *id, name: names.exact(key) })
        .collect();

    let (icon_set, icons_error) = match icons::load(game, data, schema) {
        Ok(set) => (Some(set), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let mut icon_index = crate::model::IconIndex::default();
    if let Some(set) = &icon_set {
        resolve_icons(&mut n, set);
        let used = n.items.iter().chain(&n.objects).filter_map(|e| e.icon.clone());
        let used = used.chain(n.recipes.iter().filter_map(|r| r.icon.clone()));
        let used = used.chain(n.techs.iter().flat_map(|t| [t.icon.clone(), t.lock.as_ref().and_then(|l| l.portrait.clone())]).flatten());
        icon_index.sprites = icons::referenced(set, used);
        icon_index.sheet_sizes = set.sheets.iter().map(|s| [s.width, s.height]).collect();
    }

    let data = GameData {
        model_version: MODEL_VERSION,
        game,
        fingerprint: fingerprint(root, build_id)?,
        game_build: build_id.map(str::to_string),
        unity_version: file.unity_version.clone(),
        items: n.items,
        objects: n.objects,
        groups: n.groups,
        recipes: n.recipes,
        techs: n.techs,
        branches,
        locales: names.filter_locales(locales),
        icons: icon_index,
    };
    Ok(Extracted { data, icons: icon_set, icons_error })
}

/// Icon candidates for an item id: its own sprite, then the conventions the
/// games use (`i_<id>`, carried variants, quality-less ids).
fn item_icon_candidates(id: &str) -> Vec<String> {
    let mut bases = vec![id.to_string(), id.replace(':', "_")];
    bases.extend(id.split(':').filter(|p| !p.is_empty() && p.parse::<f64>().is_err()).map(str::to_string));
    let mut out = Vec::new();
    for b in &bases {
        out.extend(["i_", "i_2h_", "i_1h_"].map(|p| format!("{p}{b}")));
    }
    for b in &bases {
        let mut cur = b.as_str();
        while let Some(i) = cur.rfind('_') {
            cur = &cur[..i];
            out.push(format!("i_{cur}"));
        }
    }
    out
}

fn object_icon_candidates(id: &str) -> Vec<String> {
    let mut out = vec![format!("i_b_{id}"), format!("i_{id}")];
    let mut cur = id;
    while let Some(i) = cur.rfind('_') {
        cur = &cur[..i];
        out.push(format!("i_b_{cur}"));
    }
    out
}

/// Replaces declared icons with ones that exist, trying fallbacks.
fn resolve_icons(n: &mut Normalized, set: &icons::IconSet) {
    let pick = |declared: Option<String>, fallbacks: Vec<String>| declared.into_iter().chain(fallbacks).find(|c| set.has(c));
    for e in &mut n.items {
        e.icon = pick(e.icon.take(), item_icon_candidates(&e.id));
    }
    for e in &mut n.objects {
        e.icon = pick(n.object_icons.get(&e.id).cloned(), object_icon_candidates(&e.id));
    }
    for r in &mut n.recipes {
        r.icon = r.icon.take().filter(|i| set.has(i));
    }
    for t in &mut n.techs {
        t.icon = t.icon.take().filter(|i| set.has(i));
        if let Some(lock) = &mut t.lock {
            let fallback = format!("portrait_icon_{}", lock.npc.trim_start_matches("npc_"));
            lock.portrait = lock.portrait.take().into_iter().chain([fallback]).find(|p| set.has(p));
        }
    }
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
    use super::{data_dir, is_test};
    use std::path::Path;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    #[test]
    fn finds_unity_data_on_every_layout() {
        let base = std::env::temp_dir().join(format!("gkc-layouts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        // Windows / Linux: <root>/<Game>_Data
        touch(&base.join("win/GK_Data/resources.assets"));
        assert_eq!(data_dir(&base.join("win")).unwrap(), base.join("win/GK_Data"));
        // macOS Steam install: <root>/<Game>.app/Contents/Resources/Data
        touch(&base.join("mac/Graveyard Keeper.app/Contents/Resources/Data/resources.assets"));
        let mac_data = base.join("mac/Graveyard Keeper.app/Contents/Resources/Data");
        assert_eq!(data_dir(&base.join("mac")).unwrap(), mac_data);
        // macOS, the .app itself chosen by hand
        assert_eq!(data_dir(&base.join("mac/Graveyard Keeper.app")).unwrap(), mac_data);
        // Anything else
        std::fs::create_dir_all(base.join("empty")).unwrap();
        assert!(data_dir(&base.join("empty")).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

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
    Entity { id: id.to_string(), name, desc, heavy: false, icon: None }
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

/// `icon_field`: the game's own icon sprite for the item, if any.
fn item_entity(names: &mut Names, item: &Value, size_field: &str, icon_field: &str) -> Entity {
    let icon = Some(text(item, icon_field)).filter(|i| !i.is_empty()).map(str::to_string);
    Entity { heavy: num(item, size_field) >= HEAVY_ITEM_SIZE, icon, ..entity(names, text(item, "id")) }
}

/// Non-empty string field.
fn opt_text(v: &Value, key: &str) -> Option<String> {
    Some(text(v, key)).filter(|s| !s.is_empty()).map(str::to_string)
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
