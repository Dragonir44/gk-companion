//! Graveyard Keeper 2: `GameBalance`.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use super::{entity, expr, item_entity, flag, list, num, referenced_objects, text, Names, Normalized};
use crate::model::{Army, Defence, Entity, Fight, Gear, Mix, Morgue, Recipe, RecipeKind, ReputationLock, Stack, TalentNode, Tech, Tool, Work};

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
/// Gardener stations and the beds they tend (garden, vineyard).
const GARDENER_BEDS: [(&str, &str); 2] = [("garden_crafter", "garden_empty"), ("vineyard_crafter", "vineyard_empty")];
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
    // Fluids are made quality-less (`embalm_red`) and stored with their
    // quality (`embalm_red:2`); embalming entries no recipe makes are tests.
    let made = |id: &str| {
        let base = id.split(':').next().unwrap_or(id);
        recipes.iter().any(|r| r.outputs.iter().any(|o| o.item == base))
    };
    let embalms = recipes
        .iter()
        .filter(|r| r.stations.iter().any(|s| s.starts_with("embalm_table")))
        .filter_map(|r| list(b, "craftDefs").iter().find(|c| text(c, "id") == r.id))
        .map(|c| text(c, "autopsyItemId").to_string())
        .filter(|id| !id.is_empty() && made(id))
        .collect();
    let collars = items
        .iter()
        .filter(|i| num(i, "redSkullsMaxCollar") as i64 != NO_COLLAR)
        .map(|i| text(i, "id").to_string())
        .collect();
    Some(Morgue { slots, organs, extras, embalms, collars })
}

/// `ItemDef.type` of swords (the player's only) and of armours.
const TYPE_SWORD: i64 = 11;
const TYPE_ARMOR: i64 = 12;

/// What zombies can wear, as the game's equipment window allows: armours,
/// and weapons that are not tools nor swords.
fn gear(i: &Value) -> Option<Gear> {
    let kind = i.get("type").and_then(Value::as_i64).unwrap_or(0);
    let weapon = flag(i, "isWeapon") && !flag(i, "isTool") && kind != TYPE_SWORD;
    if !(weapon || kind == TYPE_ARMOR) {
        return None;
    }
    // Damage is a formula: base + perk and potion terms.
    let damage = i.get("damage").and_then(|d| value(Some(d)).or_else(|| leading_number(d)));
    let positive = |f: &str| Some(num(i, f)).filter(|&v| v > 0.0);
    Some(Gear {
        armor: kind == TYPE_ARMOR,
        power: num(i, "quality") as u32,
        damage: if weapon { damage } else { None },
        range: positive("atkRange").filter(|_| weapon),
        pause: positive("atkPause").filter(|_| weapon),
    })
}

/// `4+3*PPar("buff_damage")` -> 4.
fn leading_number(d: &Value) -> Option<f64> {
    let s = text(d, "expressionString");
    let end = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    s[..end].parse().ok()
}

/// Every string in a JSON tree.
fn collect_strings<'a>(v: &'a [Value], out: &mut Vec<&'a str>) {
    for x in v {
        match x {
            Value::String(s) => out.push(s),
            Value::Array(a) => collect_strings(a, out),
            Value::Object(o) => o.values().for_each(|y| collect_strings(std::slice::from_ref(y), out)),
            _ => {}
        }
    }
}

/// `QuestCheck.triggerType` of "this battle was won".
const TRIGGER_FIGHT_WON: i64 = 39;

/// Battles (named, with a power to reach) and the military base's defences:
/// the objects built from a `<id>_fb` building, worth their quality.
fn army(b: &Value, names: &mut Names) -> Option<Army> {
    // Fight id -> quests its victory finishes.
    let mut wins: HashMap<&str, Vec<String>> = HashMap::new();
    for q in list(b, "questDefs") {
        let Some(check) = q.get("finishCheck") else { continue };
        if check.get("triggerType").and_then(Value::as_i64) == Some(TRIGGER_FIGHT_WON) {
            wins.entry(text(check, "triggerId")).or_default().push(text(q, "id").to_string());
        }
    }
    let next: HashMap<&str, &str> =
        list(b, "fightDefinitions").iter().map(|f| (text(f, "id"), text(f, "onWinNextFightId"))).collect();
    // A battle that chains into another is won with it (its follow-up
    // finishes the quest).
    let won_by = |id: &str| {
        let mut quests = Vec::new();
        let mut cur = id;
        for _ in 0..next.len() {
            quests.extend(wins.get(cur).into_iter().flatten().cloned());
            match next.get(cur) {
                Some(n) if !n.is_empty() => cur = n,
                _ => break,
            }
        }
        quests
    };
    // Battles no quest spawns nor checks are left over from development
    // (`sewer_fight_1`): the game never offers them.
    let mut quest_texts = Vec::new();
    collect_strings(list(b, "questDefs"), &mut quest_texts);
    let in_quests = |id: &str| {
        quest_texts.iter().any(|t| {
            t.match_indices(id).any(|(i, _)| !t[i + id.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
        })
    };
    // Quests that spawn a battle (as they start or end), and battles some
    // quest despawns.
    let calls = |name: &str, part: &str, q: &Value| -> Vec<String> {
        list(q, part)
            .iter()
            .filter_map(|e| text(e, "expressionString").strip_prefix(&format!("{name}(\"")))
            .filter_map(|rest| rest.split('"').next())
            .map(str::to_string)
            .collect()
    };
    let mut spawned_by: HashMap<String, Vec<String>> = HashMap::new();
    let mut despawned: std::collections::HashSet<String> = Default::default();
    for q in list(b, "questDefs") {
        for part in ["execExpressionsStart", "execExpressionsFinish"] {
            for id in calls("Spawn_FightingLvl", part, q) {
                spawned_by.entry(id).or_default().push(text(q, "id").to_string());
            }
            despawned.extend(calls("Despawn_FightingLvl", part, q));
        }
    }
    let fights: Vec<Fight> = list(b, "fightDefinitions")
        .iter()
        .filter(|f| num(f, "defencePowerLock") > 0.0 && in_quests(text(f, "id")))
        .filter_map(|f| {
            let id = text(f, "id");
            Some(Fight {
                id: id.to_string(),
                name: Some(names.exact(id)?),
                power: num(f, "defencePowerLock") as u32,
                squads: num(f, "squads") as u32,
                no_towers: flag(f, "isTowersUnavailable"),
                no_barricades: flag(f, "isBarricadesUnavailable"),
                won_by: won_by(id),
                offered_by: spawned_by.get(id).cloned().unwrap_or_default(),
                repeatable: won_by(id).is_empty() && spawned_by.contains_key(id) && !despawned.contains(id),
            })
        })
        .collect();
    let wgos: HashMap<&str, &Value> = list(b, "wgoDefs").iter().map(|w| (text(w, "id"), w)).collect();
    let defences = list(b, "buildingDefs")
        .iter()
        .filter(|d| text(d, "id").ends_with("_fb"))
        .filter_map(|d| {
            let w = wgos.get(text(d, "wgoId"))?;
            Some(Defence {
                object: text(w, "id").to_string(),
                power: value(w.get("quality")).unwrap_or(0.0) as u32,
                group: text(w, "wgoGroup").to_string(),
            })
        })
        .collect();
    (!fights.is_empty()).then_some(Army { fights, defences })
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
        e.stack = e.skulls.and(Some(num(i, "stackCount") as u32)).filter(|&n| n > 1);
        e.collar = Some(num(i, "redSkullsMaxCollar") as i64).filter(|&m| m != NO_COLLAR).map(|m| m as u32);
        let talents = super::strings(i, &["talentIds"]);
        e.tool = (flag(i, "isTool") && !talents.is_empty()).then(|| Tool {
            kind: i.get("type").and_then(Value::as_i64).unwrap_or(0),
            talents,
            bonus: num(i, "talentBonus") as u32,
            // Only the starting tools can't be destroyed.
            starter: i.get("canNotBeDestroyed").and_then(|v| v.get("pureValueBool")).and_then(Value::as_i64) == Some(1),
        });
        // Enemies' and test gear have no name.
        e.gear = gear(i).filter(|_| e.name.is_some());
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
    // Where zombies can be: stock lists and zombie views name them.
    let mut extra = storage.clone();
    extra.extend(list(b, "wgoDefs").iter().filter(|w| flag(w, "canInsertZombie")).map(|w| text(w, "id").to_string()));
    // The barracks' zombie slots have no name of their own.
    names.add_alias("fighter_container", "wz_town_guard_barracks");
    let mut objects = referenced_objects(&recipes, &extra, names);
    let work: HashMap<&str, Work> = list(b, "wgoDefs")
        .iter()
        .filter(|w| !text(w, "talent").is_empty())
        .map(|w| {
            let tool = w.get("toolAction").and_then(|t| t.get("actionableTool")).and_then(Value::as_i64).unwrap_or(0);
            (text(w, "id"), Work { talent: text(w, "talent").to_string(), tool, zombie: flag(w, "canInsertZombie"), beds: vec![] })
        })
        .collect();
    // Gardener stations have no crafts: their zombie plants in the beds of
    // the station's zone, with a shovel (the beds' tool). Not in the data.
    let mut work = work;
    for (station, bed) in GARDENER_BEDS {
        if let (Some(b), Some(w)) = (work.get(bed).cloned(), work.get_mut(station)) {
            w.tool = b.tool;
            w.beds = vec![bed.to_string()];
        }
    }
    for o in &mut objects {
        o.work = work.get(o.id.as_str()).cloned();
    }
    let morgue = morgue(b, &recipes);
    let army = army(b, names);
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

    Normalized { items, objects, groups, recipes, techs, branches, object_icons, alchemy_mixes, zones, storage, morgue, zombie_tree, army }
}
