//! Graveyard Keeper: `GameBalance` (object `game_data`).

use std::collections::BTreeMap;

use serde_json::Value;

use super::{entity, expr, item_entity, flag, list, num, referenced_objects, text, Names, Normalized};
use crate::model::{Recipe, RecipeKind, Stack, Tech};

/// Outputs that are tech points rather than items.
const POINT_IDS: [&str; 3] = ["r", "g", "b"];

/// `SmartExpression`: a pre-simplified float when the formula is constant.
fn smart(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    if flag(v, "_simplified") {
        return Some(num(v, "_simpified_float"));
    }
    expr::eval(text(v, "_expression"))
}

fn smart_expr(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let e = text(v, "_expression").trim();
    (!flag(v, "_simplified") && !e.is_empty() && e.parse::<f64>().is_err()).then(|| e.to_string())
}

fn stacks(items: &[Value]) -> Vec<Stack> {
    items
        .iter()
        .map(|i| {
            // A chance formula that depends on game state evaluates to 0:
            // kept as 0 ("random"), see Stack.chance.
            let chance = i
                .get("self_chance")
                .filter(|c| !text(c, "_expression").is_empty())
                .and_then(|c| smart(Some(c)))
                .filter(|&c| c < 1.0);
            Stack {
                item: text(i, "id").to_string(),
                count: num(i, "value"),
                group: false,
                chance,
                expr: smart_expr(i.get("min_value")),
            }
        })
        .collect()
}

/// Splits r/g/b tech points out of a craft's outputs.
fn split_points(outputs: Vec<Stack>) -> (Vec<Stack>, BTreeMap<String, f64>) {
    let mut points = BTreeMap::new();
    let items = outputs
        .into_iter()
        .filter(|s| {
            let is_point = POINT_IDS.contains(&s.item.as_str());
            if is_point {
                *points.entry(s.item.clone()).or_insert(0.0) += s.count;
            }
            !is_point
        })
        .collect();
    (items, points)
}

pub fn normalize(b: &Value, names: &mut Names) -> Normalized {
    let mut recipes = Vec::new();

    for c in list(b, "craft_data") {
        let (outputs, points) = split_points(stacks(list(c, "output")));
        recipes.push(Recipe {
            id: text(c, "id").to_string(),
            name: None,
            kind: RecipeKind::Craft,
            stations: super::strings(c, &["craft_in"]),
            inputs: stacks(list(c, "needs")),
            outputs,
            points,
            time: smart(c.get("craft_time")),
            energy: smart(c.get("energy")),
            builds: None,
            hidden: flag(c, "hidden"),
            needs_unlock: flag(c, "needs_unlock"),
        });
    }

    for c in list(b, "craft_obj_data") {
        let mut stations = super::strings(c, &["builder_ids"]);
        stations.extend(super::strings(c, &["craft_in"]));
        let out = text(c, "out_obj");
        recipes.push(Recipe {
            id: text(c, "id").to_string(),
            name: None,
            kind: RecipeKind::Building,
            stations,
            inputs: stacks(list(c, "needs")),
            outputs: vec![],
            points: BTreeMap::new(),
            time: smart(c.get("craft_time")),
            energy: smart(c.get("energy")),
            builds: (!out.is_empty()).then(|| out.to_string()),
            hidden: flag(c, "hidden"),
            needs_unlock: flag(c, "needs_unlock"),
        });
    }

    let techs = list(b, "techs_data")
        .iter()
        .map(|t| {
            let id = text(t, "id");
            let e = entity(names, id);
            let price = t.get("price").cloned().unwrap_or_default();
            let cost = super::strings(&price, &["_res_type"])
                .into_iter()
                .zip(list(&price, "_res_v").iter().map(|v| v.as_f64().unwrap_or(0.0)))
                .filter(|(_, v)| *v != 0.0)
                .collect();
            Tech {
                id: id.to_string(),
                name: e.name,
                desc: e.desc,
                branch: t.get("branch_type").and_then(Value::as_i64).unwrap_or(0),
                parents: super::strings(t, &["_parents"]),
                x: num(t, "x"),
                y: num(t, "y"),
                cost,
                // Some entries carry an `@` prefix before the craft id.
                unlocks: super::strings(t, &["crafts"]).into_iter().map(|c| c.trim_start_matches('@').to_string()).collect(),
                hidden: flag(t, "hidden") || flag(t, "invisible"),
            }
        })
        .collect();

    let items = list(b, "items_data").iter().map(|i| item_entity(names, i, "item_size")).collect();
    let objects = referenced_objects(&recipes, names);

    Normalized { items, objects, groups: BTreeMap::new(), recipes, techs }
}
