//! Graveyard Keeper 2: `GameBalance`.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{entity, expr, item_entity, flag, list, num, referenced_objects, text, Names, Normalized};
use crate::model::{Recipe, RecipeKind, Stack, Tech};

/// `BuildingDef.buildingMode` for removal entries (`*_r`), not constructions.
const BUILDING_MODE_REMOVE: i64 = 2;
const PURE_VALUE_FLOAT: i64 = 1;

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
            chance: value(i.get("chance")).filter(|&c| c > 0.0 && c < 1.0),
            expr: formula(i.get("count")),
        })
        .collect()
}

fn points(c: &Value) -> BTreeMap<String, f64> {
    [("r", "techRed"), ("g", "techGreen"), ("b", "techBlue")]
        .into_iter()
        .filter_map(|(k, f)| value(c.get(f)).filter(|&v| v != 0.0).map(|v| (k.to_string(), v)))
        .collect()
}

pub fn normalize(b: &Value, names: &mut Names) -> Normalized {
    let mut recipes = Vec::new();

    for c in list(b, "craftDefs") {
        recipes.push(Recipe {
            id: text(c, "id").to_string(),
            name: None,
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
        });
    }

    let techs = list(b, "techDefs")
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
        items.push(item_entity(names, i, "itemSize"));
    }
    // Group ids appear as ingredients, so they get names like items.
    items.extend(groups.keys().map(|g| entity(names, g)));
    let objects = referenced_objects(&recipes, names);

    Normalized { items, objects, groups, recipes, techs }
}
