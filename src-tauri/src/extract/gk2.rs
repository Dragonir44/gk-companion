//! Graveyard Keeper 2: `GameBalance`.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use super::{entity, expr, item_entity, flag, list, num, referenced_objects, text, Names, Normalized};
use crate::model::{Entity, Mix, Morgue, Recipe, RecipeKind, ReputationLock, Stack, TalentNode, Tech, Tool, Work};

/// `BuildingDef.buildingMode` for removal entries (`*_r`), not constructions.
const BUILDING_MODE_REMOVE: i64 = 2;
const PURE_VALUE_FLOAT: i64 = 1;
/// `TechDef.techDefType` of reputation gates (`lock_npc_*`).
const TECH_TYPE_REPUTATION_LOCK: i64 = 1;
/// `TechTreeTab` enum, in value order; names are `tech_tab_<name>` texts.
const TECH_TABS: [&str; 6] = ["Building", "Metallurgy", "Farming", "Theology", "Anatomy", "Cooking"];

/// Expression fields: a constant float, or a formula string.
fn value(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    if v.get("pureValueType").and_then(Value::as_i64) == Some(PURE_VALUE_FLOAT) {
        return Some(num(v, "pureValueFloat"));
    }
    expr::eval(text(v, "expressionString"))
}

fn formula(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let e = text(v, "expressionString").trim();
    let pure = v.get("pureValueType").and_then(Value::as_i64) == Some(PURE_VALUE_FLOAT);
    (!pure && !e.is_empty() && e.parse::<f64>().is_err()).then(|| e.to_string())
}

fn needs(items: &[Value]) -> Vec<Stack> {
    items
        .iter()
        .map(|i| Stack {
            item: text(i, "id").to_string(),
            count: value(i.get("count")).unwrap_or(1.0),
            // groupType != 0: the id names an item group, any member works.
            group: i.get("groupType").and_then(Value::as_i64).unwrap_or(0) != 0,
            chance: None,
            expr: formula(i.get("count")),
        })
        .collect()
}

fn outputs(v: Option<&Value>) -> Vec<Stack> {
    let Some(v) = v else { return vec![] };
    list(v, "chanceOutputItems")
        .iter()
        .map(|i| Stack {
            item: text(i, "id").to_string(),
            count: value(i.get("count")).unwrap_or(1.0),
            group: false,
            chance: chance(i.get("chance")),
            expr: formula(i.get("count")),
        })
        .collect()
}

const TOWN_CRAFT_PREFIX: &str = "town_building_craft:";
const ALCHEMY_PREFIX: &str = "alchemy:";

/// Recipe id of an alchemy formula (formula ids are their output's item id).
pub fn alchemy_id(formula: &str) -> String {
    format!("{ALCHEMY_PREFIX}{formula}")
}

fn rune_count(v: &Value, field: &str) -> u32 {
    v.get(field)
        .and_then(|f| if f.is_object() { value(Some(f)) } else { f.as_f64() })
        .map_or(0, |n| n.max(0.0) as u32)
}

fn runes(v: &Value) -> [u32; 3] {
    [rune_count(v, "runesRed"), rune_count(v, "runesGreen"), rune_count(v, "runesBlue")]
}

/// One recipe per alchemy formula, and the mixes that make each one.
fn alchemy(b: &Value) -> (Vec<Recipe>, BTreeMap<String, Vec<Mix>>) {
    let recipes = list(b, "alchemyFormulaDefs")
        .iter()
        .map(|f| {
            let id = text(f, "id");
            Recipe {
                id: alchemy_id(id),
                name: None,
                icon: None,
                kind: RecipeKind::Craft,
                stations: super::strings(f, &["craftsIn"]),
                inputs: vec![],
                outputs: vec![Stack { item: id.to_string(), count: 1.0, group: false, chance: None, expr: None }],
                points: BTreeMap::new(),
                time: None,
                energy: None,
                builds: None,
                hidden: false,
                // Formulas are unlocked by research (or revealed in game).
                needs_unlock: true,
                runes: Some(runes(f)),
                site: false,
                mastery: None,
                perks: vec![],
            }
        })
        .collect();
    let mut mixes: BTreeMap<String, Vec<Mix>> = BTreeMap::new();
    for m in list(b, "alchemyMixSourceDefs") {
        let items = ["ingredient1", "ingredient2", "ingredient3"]
            .iter()
            .map(|k| text(m, k))
            .filter(|i| !i.is_empty())
            .map(str::to_string)
            .collect();
        mixes.entry(alchemy_id(text(m, "formulaId"))).or_default().push(Mix { id: text(m, "mixId").to_string(), items });
    }
    (recipes, mixes)
}

/// Town shops and houses, one recipe per level (`Pharmacy_t1`..`_t3`,
/// named "Apothicaire", "Apothicaire II"...). Level 1 is built at a
/// signboard; higher levels upgrade the previous level's building.
fn town_buildings(b: &Value, names: &mut Names) -> Vec<Recipe> {
    let defs = list(b, "townBuildingDefs");
    let previous: HashMap<&str, &str> = defs
        .iter()
        .filter(|t| !text(t, "lvlUpId").is_empty())
        .map(|t| (text(t, "lvlUpId"), text(t, "id")))
        .collect();
    defs.iter()
        .map(|t| {
            let id = text(t, "id");
            let mut stations = super::strings(t, &["craftsIn"]);
            // Level 1 is built on a town plot: a construction site.
            let on_plot = !stations.is_empty();
            if let Some(prev) = previous.get(id) {
                stations = vec![prev.to_string()];
            }
            Recipe {
                id: id.to_string(),
                name: names.exact(id),
                icon: super::opt_text(t, "iconId"),
                kind: RecipeKind::Building,
                stations,
                inputs: needs(list(t, "needItems")),
                outputs: vec![],
                points: BTreeMap::new(),
                time: None,
                energy: None,
                builds: None,
                hidden: false,
                needs_unlock: flag(t, "isNeedsUnlock"),
                runes: None,
                site: on_plot && !previous.contains_key(id),
                mastery: None,
                perks: vec![],
            }
        })
        .collect()
}

/// One-off world jobs: the craft replaces its object (repairs) or removes
/// it (blockages). Plantings also replace their plot but are repeated.
fn is_site(c: &Value) -> bool {
    let changes_world = !text(c, "replaceWgoId").is_empty() || flag(c, "isObjDestroyCraft");
    changes_world && !text(c, "id").contains("planting")
}

/// Characters are world objects with the same id; they carry a portrait.
fn reputation_lock(t: &Value, names: &mut Names, portraits: &HashMap<&str, &str>) -> Option<ReputationLock> {
    let rv = t.get("wgoRepLock").map(|l| list(l, "resValues")).unwrap_or(&[]).first()?;
    let npc = text(rv, "type").to_string();
    let portrait = portraits.get(npc.as_str()).map(|p| p.to_string());
    Some(ReputationLock { name: names.exact(&npc), value: num(rv, "value"), portrait, npc })
}

/// None for a guaranteed output. A chance formula that depends on game
/// state (`0.5*WorkerPar("perk_alchemist")`) evaluates to 0 without perks:
/// still a chance (0 = "random"), never a guaranteed output.
fn chance(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    let set = v.get("pureValueType").and_then(Value::as_i64) == Some(PURE_VALUE_FLOAT)
        || !text(v, "expressionString").trim().is_empty();
    if !set {
        return None;
    }
    let c = value(Some(v)).unwrap_or(0.0);
    (c < 1.0).then_some(c.max(0.0))
}

fn points(c: &Value) -> BTreeMap<String, f64> {
    [("r", "techRed"), ("g", "techGreen"), ("b", "techBlue")]
        .into_iter()
        .filter_map(|(k, f)| value(c.get(f)).filter(|&v| v != 0.0).map(|v| (k.to_string(), v)))
        .collect()
}

/// `redSkullsMaxCollar` of items that are not collars (C# `int.MinValue`).
const NO_COLLAR: i64 = i32::MIN as i64;
const MISTAKE_PREFIX: &str = "surgeon_mistake_";
const UI_TEXTS: [&str; 5] = ["ui_skulls_red", "ui_skulls_white", "zombie_collar", "ui_mastery", "ui_embalming_effect"];

/// Organ slots in body order (`BodyDef.parts`: skin, bones, skull...), the
/// optional parts, embalming fluids and collars. Surgeon mistakes (left in
/// a slot when an extraction fails) are listed with their slot's organs.
fn morgue(b: &Value, recipes: &[Recipe]) -> Option<Morgue> {
    let items = list(b, "itemDefs");
    let groups = |i: &Value| super::strings(i, &["itemGroupIds"]);
    let is_part = |i: &Value| groups(i).iter().any(|g| g == "bodypart");
    let slot_of = |i: &Value| groups(i).into_iter().find(|g| g.starts_with("gr_"));
    let mut slots: Vec<String> = Vec::new();
    let mut organs: Vec<Vec<String>> = Vec::new();
    let mut extras = Vec::new();
    for i in items.iter().filter(|i| is_part(i) && !flag(i, "isOrganMistake")) {
        let (id, slot) = (text(i, "id").to_string(), slot_of(i));
        match slot.filter(|_| flag(i, "isMainOrgan")) {
            Some(slot) => match slots.iter().position(|s| *s == slot) {
                Some(k) => organs[k].push(id),
                None => {
                    slots.push(slot);
                    organs.push(vec![id]);
                }
            },
            None => extras.push(id),
        }
    }
    if slots.is_empty() {
        return None;
    }
    // `surgeon_mistake_skull` goes in `gr_skulls`, `_skin` in `gr_skin`.
    for i in items.iter().filter(|i| flag(i, "isOrganMistake")) {
        let id = text(i, "id");
        let part = id.trim_start_matches(MISTAKE_PREFIX);
        match slots.iter().position(|s| s == &format!("gr_{part}") || s == &format!("gr_{part}s")) {
            Some(k) => organs[k].push(id.to_string()),
            None => extras.push(id.to_string()),
        }
    }
    let embalms = recipes
        .iter()
        .filter(|r| r.stations.iter().any(|s| s.starts_with("embalm_table")))
        .filter_map(|r| list(b, "craftDefs").iter().find(|c| text(c, "id") == r.id))
        .map(|c| text(c, "autopsyItemId").to_string())
        .filter(|id| !id.is_empty())
        .collect();
    let collars = items
        .iter()
        .filter(|i| num(i, "redSkullsMaxCollar") as i64 != NO_COLLAR)
        .map(|i| text(i, "id").to_string())
        .collect();
    Some(Morgue { slots, organs, extras, embalms, collars })
}

/// `TalentLevelUpDef`s flagged `isZombiePerk`; perks come from `perkDefs`.
fn zombie_tree(b: &Value, names: &mut Names) -> Vec<TalentNode> {
    let perks: HashMap<&str, &Value> = list(b, "perkDefs").iter().map(|p| (text(p, "id"), p)).collect();
    list(b, "talentLevelUpDefs")
        .iter()
        .filter(|t| flag(t, "isZombiePerk"))
        .map(|t| {
            let perk = super::opt_text(t, "linkedPerk");
            let def = perk.as_deref().and_then(|p| perks.get(p));
            let named = perk.as_deref().map(|p| entity(names, p));
            let cost = [("r", "techRed"), ("g", "techGreen"), ("b", "techBlue")]
                .into_iter()
                .map(|(k, f)| (k.to_string(), num(t, f)))
                .filter(|(_, v)| *v != 0.0)
                .collect();
            TalentNode {
                id: text(t, "id").to_string(),
                talent: text(t, "talentId").to_string(),
                parents: super::strings(t, &["parents"]),
                x: num(t, "posX"),
                y: num(t, "posY"),
                cost,
                mastery: num(t, "talentValueAdd") as u32,
                name: named.as_ref().and_then(|e| e.name.clone()),
                desc: named.and_then(|e| e.desc),
                icon: super::opt_text(t, "icon"),
                mastery_bonus: def.map_or(0, |p| num(p, "craftMasteryBonus") as u32),
                // Both make the craft faster: fewer ticks to do, or a head start.
                speed_bonus: def.map_or(0, |p| (num(p, "craftStartTicks") - num(p, "craftTotalProgressTicksBonus")) as i32),
                perk,
            }
        })
        .collect()
}

pub fn normalize(b: &Value, names: &mut Names) -> Normalized {
    let mut recipes = Vec::new();

    for c in list(b, "craftDefs") {
        recipes.push(Recipe {
            id: text(c, "id").to_string(),
            name: None,
            icon: super::opt_text(c, "iconId"),
            kind: RecipeKind::Craft,
            stations: super::strings(c, &["craftsIn"]),
            inputs: needs(list(c, "needItems")),
            outputs: outputs(c.get("outputItems")),
            points: points(c),
            time: value(c.get("duration")),
            // energyPerTick's tick length isn't known yet; not exposed.
            energy: None,
            builds: None,
            hidden: flag(c, "isHidden"),
            needs_unlock: flag(c, "isNeedsUnlock"),
            runes: None,
            site: is_site(c),
            mastery: Some(num(c, "talentLock") as u32).filter(|&m| m > 0),
            perks: super::strings(c, &["linkedPerks"]),
        });
    }

    for c in list(b, "buildingDefs") {
        if c.get("buildingMode").and_then(Value::as_i64) == Some(BUILDING_MODE_REMOVE) {
            continue;
        }
        let out = text(c, "wgoId");
        recipes.push(Recipe {
            id: text(c, "id").to_string(),
            name: None,
            icon: super::opt_text(c, "buildResultIcon"),
            kind: RecipeKind::Building,
            stations: super::strings(c, &["buildsIn"]),
            inputs: needs(list(c, "needItems")),
            outputs: outputs(c.get("outputItems")),
            points: BTreeMap::new(),
            time: None,
            energy: None,
            builds: (!out.is_empty()).then(|| out.to_string()),
            hidden: false,
            needs_unlock: flag(c, "isNeedsUnlock"),
            runes: None,
            site: false,
            mastery: None,
            perks: vec![],
        });
    }

    // Alchemy: the lab makes formulas from any mix whose runes add up.
    // Crafts with a formula's name but no station (rich elixir + cinnabar
    // for the miracle elixir...) are not how the game makes them: hidden.
    let formulas: Vec<&str> = list(b, "alchemyFormulaDefs").iter().map(|f| text(f, "id")).collect();
    for r in recipes.iter_mut().filter(|r| r.stations.is_empty() && formulas.contains(&r.id.as_str())) {
        r.hidden = true;
    }
    let (alchemy, alchemy_mixes) = alchemy(b);
    recipes.extend(alchemy);

    // Town signboards (`t_b_signboard_house`) share the texts of the plots
    // they stand on (`repair_sign_house`: "Logement").
    for plot in ["house", "shop", "yard"] {
        names.add_alias(&format!("t_b_signboard_{plot}"), &format!("repair_sign_{plot}"));
    }
    recipes.extend(town_buildings(b, names));
    // Each town building also exists as a nameless `town_building_craft:<id>` craft.
    for r in recipes.iter_mut().filter(|r| r.id.starts_with(TOWN_CRAFT_PREFIX)) {
        r.hidden = true;
    }

    let portraits: HashMap<&str, &str> = list(b, "wgoDefs")
        .iter()
        .map(|w| (text(w, "id"), text(w, "portrait")))
        .filter(|(_, p)| !p.is_empty())
        .collect();
    let techs: Vec<Tech> = list(b, "techDefs")
        .iter()
        .map(|t| {
            let id = text(t, "id");
            let e = entity(names, id);
            let cost = [("r", "redSpheresPrice"), ("g", "greenSpheresPrice"), ("b", "blueSpheresPrice")]
                .into_iter()
                .map(|(k, f)| (k.to_string(), num(t, f)))
                .filter(|(_, v)| *v != 0.0)
                .collect();
            let mut unlocks = super::strings(t, &["craftsAfterUnlock"]);
            unlocks.extend(super::strings(t, &["alchemyFormulasAfterUnlock"]).iter().map(|f| alchemy_id(f)));
            unlocks.extend(super::strings(t, &["buildingsAfterUnlock"]));
            Tech {
                id: id.to_string(),
                name: e.name,
                desc: e.desc,
                branch: t.get("tab").and_then(Value::as_i64).unwrap_or(0),
                parents: super::strings(t, &["parents"]),
                x: num(t, "posX"),
                y: num(t, "posY"),
                cost,
                unlocks,
                hidden: flag(t, "hiddenAtStart"),
                icon: super::opt_text(t, "customIconId"),
                lock: (t.get("techDefType").and_then(Value::as_i64) == Some(TECH_TYPE_REPUTATION_LOCK))
                    .then(|| reputation_lock(t, names, &portraits))
                    .flatten(),
            }
        })
        .collect();

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut items = Vec::new();
    for i in list(b, "itemDefs") {
        let id = text(i, "id");
        for g in super::strings(i, &["itemGroupIds"]) {
            groups.entry(g).or_default().push(id.to_string());
        }
        let mut e = item_entity(names, i, "itemSize", "iconId");
        e.runes = Some(runes(i)).filter(|r| r.iter().any(|&n| n > 0));
        e.skulls = Some([num(i, "redSkulls") as i32, num(i, "whiteSkulls") as i32]).filter(|s| s != &[0, 0]);
        e.collar = Some(num(i, "redSkullsMaxCollar") as i64).filter(|&m| m != NO_COLLAR).map(|m| m as u32);
        let talents = super::strings(i, &["talentIds"]);
        e.tool = (flag(i, "isTool") && !talents.is_empty()).then(|| Tool {
            kind: i.get("type").and_then(Value::as_i64).unwrap_or(0),
            talents,
            bonus: num(i, "talentBonus") as u32,
        });
        items.push(e);
    }
    // Group ids appear as ingredients, so they get names like items.
    items.extend(groups.keys().map(|g| entity(names, g)));
    // Zone names are `wz_<zone>`; storage objects get names for stock lists.
    let zones = list(b, "worldZoneDefs")
        .iter()
        .map(|z| {
            let id = text(z, "id");
            // Town districts have no zone name, only their battle's
            // (`wz_fight_A3_1` -> `fight_A3_1`: "Clear the Market Square").
            let key = match id.strip_prefix("wz_") {
                Some(battle) => battle.to_string(),
                None => format!("wz_{id}"),
            };
            Entity { name: names.exact(&key), ..entity(names, id) }
        })
        .collect();
    let storage: Vec<String> = list(b, "wgoDefs")
        .iter()
        .map(|w| text(w, "id"))
        .filter(|id| crate::save::inventory::is_storage(id))
        .map(str::to_string)
        .collect();
    let mut objects = referenced_objects(&recipes, &storage, names);
    let work: HashMap<&str, Work> = list(b, "wgoDefs")
        .iter()
        .filter(|w| !text(w, "talent").is_empty())
        .map(|w| {
            let tool = w.get("toolAction").and_then(|t| t.get("actionableTool")).and_then(Value::as_i64).unwrap_or(0);
            (text(w, "id"), Work { talent: text(w, "talent").to_string(), tool, zombie: flag(w, "canInsertZombie") })
        })
        .collect();
    for o in &mut objects {
        o.work = work.get(o.id.as_str()).cloned();
    }
    let morgue = morgue(b, &recipes);
    let zombie_tree = zombie_tree(b, names);
    // Zombies are named from a pool of texts; the save says which one.
    names.keep_prefixed("zombie_name_");
    // The game's own labels for the morgue and zombie views.
    for key in UI_TEXTS {
        names.exact(key);
    }

    let object_icons = list(b, "wgoDefs")
        .iter()
        .filter_map(|w| Some((text(w, "id").to_string(), super::opt_text(w, "craftIconId")?)))
        .collect();

    let mut tabs: Vec<i64> = techs.iter().map(|t| t.branch).collect();
    tabs.sort_unstable();
    tabs.dedup();
    let branches = tabs
        .into_iter()
        .map(|id| (id, TECH_TABS.get(id as usize).map(|n| format!("tech_tab_{n}")).unwrap_or_default()))
        .collect();

    Normalized { items, objects, groups, recipes, techs, branches, object_icons, alchemy_mixes, zones, storage, morgue, zombie_tree }
}
