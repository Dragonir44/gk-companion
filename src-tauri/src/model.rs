//! Game-agnostic data model shared by both games and sent to the frontend.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Bump when the model changes, so stale caches are re-extracted.
pub const MODEL_VERSION: u32 = 22;

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
    /// The in-game world map (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<WorldMap>,
    /// What bodies are made of, for skull counts (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morgue: Option<Morgue>,
    /// The zombies' own talent tree (gk2): one tree per talent, shared by
    /// every zombie.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zombie_tree: Vec<TalentNode>,
    /// Battles and the defences that count for them (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army: Option<Army>,
}

/// What a battle needs and what counts towards it. A battle's power is
/// the military base's defences (unless the battle rules them out) plus
/// the power of the squads sent, up to `Fight::squads`. A fighter's power
/// is its weapon's plus its armour's (`Gear::power`), 0 without both.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Army {
    /// Battles in game order (named ones with a power requirement).
    pub fights: Vec<Fight>,
    /// Defences built at the military base: object id -> power and kind.
    pub defences: Vec<Defence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fight {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Least power to start it.
    pub power: u32,
    pub squads: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_towers: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_barricades: bool,
    /// Quests finished by winning it, or the battle it chains into: won
    /// once one of them is finished in the save.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub won_by: Vec<String>,
    /// Quests that make it appear (as they start or end): offered once one
    /// is active or finished.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered_by: Vec<String>,
    /// Nothing ends it once offered (the arena's last round): it can be
    /// fought again and again, and is never "won".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeatable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Defence {
    pub object: String,
    pub power: u32,
    /// `towers` or `barricades` (the game's object group).
    pub group: String,
}

/// Body parts by role. Skull counts are on the items (`Entity::skulls`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Morgue {
    /// Organ slots, as item groups (`gr_skin`...), one organ each.
    pub slots: Vec<String>,
    /// Organs per slot, same order as `slots`.
    pub organs: Vec<Vec<String>>,
    /// Optional parts a body may carry (flesh, fat, blood).
    pub extras: Vec<String>,
    /// Embalming fluids, applied at the embalming table.
    pub embalms: Vec<String>,
    /// Zombie collars (`Entity::collar`).
    pub collars: Vec<String>,
}

/// A node of the zombie talent tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TalentNode {
    pub id: String,
    /// Talent (`talent_orange`...) whose tree holds the node.
    pub talent: String,
    pub parents: Vec<String>,
    pub x: f64,
    pub y: f64,
    /// Points the zombie spends, keyed r, g, b. Each node also takes one
    /// of the zombie's red skulls.
    pub cost: BTreeMap<String, f64>,
    /// Mastery added to the tree's talent.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mastery: u32,
    /// Perk granted (its name and description name the node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Perk effects on the crafts that list the perk (`Recipe::perks`):
    /// extra mastery, and work ticks it saves on each craft.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mastery_bonus: u32,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub speed_bonus: i32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

fn is_zero_i(v: &i32) -> bool {
    *v == 0
}

/// The world map: an image and where the world falls on it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMap {
    /// Sprite of the image in the icon sheets.
    pub sprite: String,
    /// Ground rectangle (`[x, z]`) the image covers, left to right and
    /// bottom to top: the game maps positions linearly between them.
    pub world_min: [f32; 2],
    pub world_max: [f32; 2],
    /// Interiors are drawn at a fixed point: zone id -> position on the
    /// image, as fractions from its top-left corner.
    pub zones: BTreeMap<String, [f32; 2]>,
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
    /// Skulls `[red, white]` the item brings to a body (gk2 organs,
    /// embalming fluids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skulls: Option<[i32; 2]>,
    /// How many of a body part or fluid one body slot holds (gk2, items with skulls).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<u32>,
    /// Zombie collar: most red skulls the zombie keeps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collar: Option<u32>,
    /// Tool: the mastery it gives a worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
    /// Work station: the talent its crafts use (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<Work>,
    /// Weapon or armour a zombie fighter can wear (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gear: Option<Gear>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Gear {
    /// Armour; a weapon otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub armor: bool,
    /// What it adds to its wearer's power (the item's quality).
    pub power: u32,
    /// Weapons: base damage (before perks and potions), reach, pause
    /// between attacks in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Tool type; a station takes tools of its type (`Work::tool`).
    pub kind: i64,
    /// Talents it adds `bonus` mastery to.
    pub talents: Vec<String>,
    pub bonus: u32,
    /// The player's starting (rusty) tools: zombies can't hold them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub starter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub talent: String,
    /// Tool type used here; `ANY_TOOL` takes any tool of the talent.
    pub tool: i64,
    /// A zombie can be assigned to it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub zombie: bool,
    /// Objects it works on instead of itself: a gardener's station tends
    /// the beds of its zone (their crafts and tool are its own).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beds: Vec<String>,
}

/// `Work::tool` of stations worked with whatever tool matches the talent.
pub const ANY_TOOL: i64 = 10;

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
    /// A one-off job on a world object (repair, clear a blockage, build on
    /// a town plot): listed as a construction site (gk2).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub site: bool,
    /// Mastery needed in the station's talent (gk2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mastery: Option<u32>,
    /// Perks that help with this craft (player and zombie perks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub perks: Vec<String>,
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
            site: false,
            mastery: None,
            perks: vec![],
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
            items: vec![Entity {
                id: "nails".into(),
                name: None,
                desc: None,
                heavy: false,
                icon: None,
                runes: None,
                skulls: None,
                stack: None,
                collar: None,
                tool: None,
                work: None,
                gear: None,
            }],
            objects: vec![],
            groups: BTreeMap::new(),
            recipes: vec![recipe],
            techs: vec![tech],
            branches: vec![Branch { id: 0, name: None }],
            zones: vec![],
            locales: BTreeMap::new(),
            icons: IconIndex::default(),
            alchemy_mixes: BTreeMap::new(),
            map: Some(WorldMap {
                sprite: "ui:world_map".into(),
                world_min: [-70.0, -160.0],
                world_max: [193.0, 156.0],
                zones: BTreeMap::from([("home".into(), [0.25, 0.4])]),
            }),
            morgue: None,
            army: None,
            zombie_tree: vec![TalentNode {
                id: "z".into(),
                talent: "talent_orange".into(),
                parents: vec![],
                x: 0.0,
                y: 0.0,
                cost: BTreeMap::new(),
                mastery: 0,
                perk: None,
                name: None,
                desc: None,
                icon: None,
                mastery_bonus: 0,
                speed_bonus: 0,
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("\"points\""), "empty fields are omitted");
        let back: GameData = serde_json::from_str(&json).expect("cache must read back");
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
