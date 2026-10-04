//! Game-agnostic data model shared by both games and sent to the frontend.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Bump when the model changes, so stale caches are re-extracted.
pub const MODEL_VERSION: u32 = 5;

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
    pub unity_version: String,
    pub items: Vec<Entity>,
    /// World objects: stations, buildings.
    pub objects: Vec<Entity>,
    /// Item group id -> member item ids (GK2 "any of" ingredients).
    pub groups: BTreeMap<String, Vec<String>>,
    pub recipes: Vec<Recipe>,
    pub techs: Vec<Tech>,
    /// Language -> text key -> text. Only keys referenced by entities.
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub icons: IconIndex,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Carried overhead (logs, stone blocks): the games often give the
    /// heavy item and its hand-held pieces the same name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub heavy: bool,
    /// Icon sprite name (see `IconIndex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub points: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy: Option<f64>,
    /// Object built (building recipes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builds: Option<String>,
    pub hidden: bool,
    pub needs_unlock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    /// Item id, or group id when `group` is set.
    pub item: String,
    pub count: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    /// Drop chance in ]0, 1[; 0 when it depends on game state (luck...).
    /// None for guaranteed outputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chance: Option<f64>,
    /// Original formula when the count depends on game state (perks...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tech {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Tree tab / branch.
    pub branch: i64,
    pub parents: Vec<String>,
    pub x: f64,
    pub y: f64,
    pub cost: BTreeMap<String, f64>,
    /// Recipe ids unlocked.
    pub unlocks: Vec<String>,
    pub hidden: bool,
}
