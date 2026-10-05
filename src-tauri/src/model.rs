//! Game-agnostic data model shared by both games and sent to the frontend.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Bump when the model changes, so stale caches are re-extracted.
pub const MODEL_VERSION: u32 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameId {
    Gk1,
    Gk2,
}

impl GameId {
    pub fn as_str(self) -> &'static str {
        match self {
            GameId::Gk1 => "gk1",
            GameId::Gk2 => "gk2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub model_version: u32,
    pub game: GameId,
    /// Identifies the game files this was extracted from.
    pub fingerprint: String,
    /// Steam build id of the game version extracted, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_build: Option<String>,
    pub unity_version: String,
    pub items: Vec<Entity>,
    /// World objects: stations, buildings.
    pub objects: Vec<Entity>,
    /// Item group id -> member item ids (GK2 "any of" ingredients).
    pub groups: BTreeMap<String, Vec<String>>,
    pub recipes: Vec<Recipe>,
    pub techs: Vec<Tech>,
    /// Research tree tabs, in game order.
    pub branches: Vec<Branch>,
    /// World zones (where stored items are), gk2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<Entity>,
    /// Language -> text key -> text. Only keys referenced by entities.
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub icons: IconIndex,
    /// Alchemy recipe id -> the ingredient mixes that make it (gk2: every
    /// valid combination, precomputed by the game).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub alchemy_mixes: BTreeMap<String, Vec<Mix>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    /// The game's mix id (`mix:clay:flour:fragrance`), as listed in saves.
    pub id: String,
    /// One of each.
    pub items: Vec<String>,
}

/// Icon sprites, cut from sheet images written next to the cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconIndex {
    /// Sheet image file names, relative to the cache directory.
    pub sheets: Vec<String>,
    /// Sheet sizes, `[width, height]`, same order as `sheets`.
    pub sheet_sizes: Vec<[u32; 2]>,
    /// Sprite name -> `[sheet, x, y, width, height]` (top-left origin).
    pub sprites: BTreeMap<String, [u32; 5]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    /// Localization key of the name; None when the game has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Carried overhead (logs, stone blocks): the games often give the
    /// heavy item and its hand-held pieces the same name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub heavy: bool,
    /// Icon sprite name (see `IconIndex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Alchemy runes `[red, green, blue]` the item brings to a mix (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runes: Option<[u32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecipeKind {
    Craft,
    Building,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    /// The craft's own name key ("Repair the zombie carousel"), when the
    /// game has one; otherwise it is named after what it makes or acts on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Own icon (world crafts, buildings); otherwise use what it makes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub kind: RecipeKind,
    /// Object ids where this recipe can be made.
    pub stations: Vec<String>,
    pub inputs: Vec<Stack>,
    pub outputs: Vec<Stack>,
    /// Tech points produced (gk1 r/g/b crafts) — keyed r, g, b, ...
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub points: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<f64>,
    /// Object built (building recipes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builds: Option<String>,
    pub hidden: bool,
    pub needs_unlock: bool,
    /// Alchemy formula: runes `[red, green, blue]` the mix must total. Its
    /// ingredients are one of `GameData::alchemy_mixes`, not `inputs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runes: Option<[u32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    /// Item id, or group id when `group` is set.
    pub item: String,
    pub count: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    /// Drop chance in ]0, 1[; 0 when it depends on game state (luck...).
    /// None for guaranteed outputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chance: Option<f64>,
    /// Original formula when the count depends on game state (perks...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Value of `Tech::branch`.
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tech {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Tree tab / branch.
    pub branch: i64,
    pub parents: Vec<String>,
    pub x: f64,
    pub y: f64,
    pub cost: BTreeMap<String, f64>,
    /// Recipe ids unlocked.
    pub unlocks: Vec<String>,
    /// Hidden until revealed in game: a spoiler.
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Not a research but a gate: reputation needed with a character (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<ReputationLock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReputationLock {
    /// Character id; `name` is the key of its name.
    pub npc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value: f64,
    /// Portrait sprite of the character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fields left out when empty must read back: the cache is written
    /// with them omitted, and a cache that fails to load is re-extracted
    /// on every launch.
    #[test]
    fn game_data_round_trips_with_omitted_fields() {
        let stack = Stack { item: "nails".into(), count: 4.0, group: false, chance: None, expr: None };
        let recipe = Recipe {
            id: "nails".into(),
            name: None,
            icon: None,
            kind: RecipeKind::Craft,
            stations: vec![],
            inputs: vec![stack.clone()],
            outputs: vec![stack],
            points: BTreeMap::new(),
            time: None,
            energy: None,
            builds: None,
            hidden: false,
            needs_unlock: false,
            runes: None,
        };
        let tech = Tech {
            id: "t".into(),
            name: None,
            desc: None,
            branch: 0,
            parents: vec![],
            x: 0.0,
            y: 0.0,
            cost: BTreeMap::new(),
            unlocks: vec![],
            hidden: false,
            icon: None,
            lock: None,
        };
        let data = GameData {
            model_version: MODEL_VERSION,
            game: GameId::Gk2,
            fingerprint: "fp".into(),
            game_build: None,
            unity_version: "6000".into(),
            items: vec![Entity { id: "nails".into(), name: None, desc: None, heavy: false, icon: None, runes: None }],
            objects: vec![],
            groups: BTreeMap::new(),
            recipes: vec![recipe],
            techs: vec![tech],
            branches: vec![Branch { id: 0, name: None }],
            zones: vec![],
            locales: BTreeMap::new(),
            icons: IconIndex::default(),
            alchemy_mixes: BTreeMap::new(),
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("\"points\""), "empty fields are omitted");
        let back: GameData = serde_json::from_str(&json).expect("cache must read back");
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
