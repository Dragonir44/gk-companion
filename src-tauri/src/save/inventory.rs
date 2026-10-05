//! World objects of a gk2 save: where they are, and the items stored in
//! chests, sheds, containers and the player's inventory.
//!
//! World objects live in `worldData/gameSceneDataList/*/wgoDataList/*`;
//! each has an `inventory` whose items (`inventory/*` nodes with `id` and
//! `count`) may themselves hold an inventory (bags). Workbench contents
//! (`craftInventory`) and zombies' loads are not stock and are skipped.

use std::collections::BTreeMap;

use serde::Serialize;

use super::odin::{Entry, Reader};
use super::SaveError;

/// Storage objects: everything else (graves, gardens, NPCs, tables) holds
/// things that are not stock.
const STORAGE_HINTS: [&str; 5] = ["chest", "shed", "container", "storage", "pallet"];
/// Game resources stored as items (fire in a fireplace, faith, science...).
const PSEUDO_ITEMS: [&str; 7] = ["empty", "inventory", "fire", "faith", "science", "town_gratitude", "money"];
/// Subtrees that are not storage.
const SKIPPED: [&str; 3] = ["craftInventory", "zombieItem", "toolBeltInventory"];
const PLAYER: &str = "player";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    /// World object id (`chest_rough`), or `player`.
    pub object: String,
    /// World zone id (`alchemy_lab`; its name is the text `wz_<zone>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    pub items: BTreeMap<String, u32>,
}

pub fn is_storage(object: &str) -> bool {
    STORAGE_HINTS.iter().any(|h| object.contains(h))
}

/// A world object instance: construction sites, chests, buildings...
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldObject {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Ground position `[x, z]` (y is height).
    pub pos: [f32; 2],
}

/// Objects parked far off the map (pooled, hidden).
const PARKED: f32 = -500.0;

#[derive(Default)]
struct Frame {
    name: Option<String>,
    array: bool,
    /// Item node being read: its id and count.
    item_id: Option<String>,
    item_count: i64,
}

/// World objects and stored items.
pub struct World {
    pub containers: Vec<Container>,
    pub objects: Vec<WorldObject>,
}

pub fn containers(bytes: &[u8]) -> Result<Vec<Container>, SaveError> {
    Ok(world(bytes)?.containers)
}

pub fn world(bytes: &[u8]) -> Result<World, SaveError> {
    let mut r = Reader::new(bytes);
    let mut stack: Vec<Frame> = Vec::new();
    // World object being read: (stack depth of its node, object id, zone).
    let mut wgo: Option<(usize, String, Option<String>)> = None;
    // Its `position` floats (first three: x, height, z).
    let mut pos: Vec<f64> = Vec::new();
    let mut objects = Vec::new();
    let mut current: BTreeMap<String, u32> = BTreeMap::new();
    let mut player: BTreeMap<String, u32> = BTreeMap::new();
    let mut out = Vec::new();

    let named = |stack: &[Frame], n: &str| stack.iter().any(|f| f.name.as_deref() == Some(n));
    loop {
        match r.next_entry()? {
            Entry::Start(name, _) => {
                // An element of `wgoDataList` starts a world object.
                let parent_is = |n: &str| {
                    stack.len() >= 2 && stack[stack.len() - 1].array && stack[stack.len() - 2].name.as_deref() == Some(n)
                };
                if name.is_none() && parent_is("wgoDataList") {
                    wgo = Some((stack.len(), String::new(), None));
                    current.clear();
                    pos.clear();
                }
                stack.push(Frame { name, ..Default::default() });
            }
            Entry::StartArray => stack.push(Frame { array: true, ..Default::default() }),
            Entry::Str(Some(field), value) => {
                let depth = stack.len();
                if let Some((d, id, zone)) = &mut wgo {
                    if depth == *d + 1 {
                        match field.as_str() {
                            "id" => *id = value.clone(),
                            "worldZoneDataId" if !value.is_empty() => *zone = Some(value.clone()),
                            _ => {}
                        }
                    }
                }
                if field == "id" {
                    if let Some(top) = stack.last_mut() {
                        top.item_id = Some(value);
                    }
                }
            }
            Entry::Float(_, v) => {
                // Direct children of the object's `position` node.
                if let Some((d, ..)) = &wgo {
                    let in_position = stack.len() == *d + 2 && stack.last().and_then(|f| f.name.as_deref()) == Some("position");
                    if in_position && pos.len() < 3 {
                        pos.push(v);
                    }
                }
            }
            Entry::Int(Some(field), v) if field == "count" => {
                if let Some(top) = stack.last_mut() {
                    top.item_count = v;
                }
            }
            Entry::End | Entry::EndArray => {
                let Some(frame) = stack.pop() else { break };
                // An item node: unnamed, inside an array inside an `inventory`.
                let in_inventory = stack.len() >= 2
                    && stack[stack.len() - 1].array
                    && stack[stack.len() - 2].name.as_deref() == Some("inventory");
                if frame.name.is_none() && !frame.array && in_inventory && !SKIPPED.iter().any(|s| named(&stack, s)) {
                    if let (Some(id), n) = (frame.item_id, frame.item_count) {
                        if n > 0 && !PSEUDO_ITEMS.contains(&id.as_str()) {
                            let target = if named(&stack, "playerData") {
                                Some(&mut player)
                            } else if wgo.is_some() {
                                Some(&mut current)
                            } else {
                                None
                            };
                            if let Some(t) = target {
                                *t.entry(id).or_insert(0) += n as u32;
                            }
                        }
                    }
                }
                if let Some((d, id, zone)) = &wgo {
                    if stack.len() == *d {
                        if is_storage(id) && !current.is_empty() {
                            out.push(Container { object: id.clone(), zone: zone.clone(), items: std::mem::take(&mut current) });
                        }
                        if let [x, _, z] = pos[..] {
                            if x as f32 > PARKED && !id.is_empty() {
                                objects.push(WorldObject { id: id.clone(), zone: zone.clone(), pos: [x as f32, z as f32] });
                            }
                        }
                        wgo = None;
                    }
                }
            }
            Entry::EndOfStream => break,
            _ => {}
        }
    }
    if !player.is_empty() {
        out.insert(0, Container { object: PLAYER.into(), zone: None, items: player });
    }
    Ok(World { containers: out, objects })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s8(s: &str) -> Vec<u8> {
        let mut v = vec![0u8];
        v.extend((s.len() as i32).to_le_bytes());
        v.extend(s.as_bytes());
        v
    }
    /// Named struct start (0x03) with no type (0x2E).
    fn start(b: &mut Vec<u8>, name: Option<&str>) {
        match name {
            Some(n) => {
                b.push(0x03);
                b.extend(s8(n));
            }
            None => b.push(0x04),
        }
        b.push(0x2E);
    }
    fn arr(b: &mut Vec<u8>) {
        b.push(0x06);
        b.extend(0i64.to_le_bytes());
    }
    fn str_field(b: &mut Vec<u8>, name: &str, v: &str) {
        b.push(0x27);
        b.extend(s8(name));
        b.extend(s8(v));
    }
    fn item(b: &mut Vec<u8>, id: &str, count: i32) {
        start(b, None);
        str_field(b, "id", id);
        b.push(0x17);
        b.extend(s8("count"));
        b.extend(count.to_le_bytes());
        b.push(0x05);
    }
    /// `<name>: { inventoryItem: { inventory: [items] } }`
    fn inventory(b: &mut Vec<u8>, name: &str, items: &[(&str, i32)]) {
        start(b, Some(name));
        start(b, Some("inventoryItem"));
        start(b, Some("inventory"));
        arr(b);
        for (id, n) in items {
            item(b, id, *n);
        }
        b.extend([0x07, 0x05, 0x05, 0x05]);
    }

    #[test]
    fn reads_storage_and_player_stock() {
        let mut b = Vec::new();
        start(&mut b, None);
        start(&mut b, Some("wgoDataList"));
        arr(&mut b);
        for (id, zone, items) in [
            ("chest_rough", "mine", vec![("coal", 990), ("fire", 3)]),
            ("grave_ground", "graveyard", vec![("body", 1)]),
        ] {
            start(&mut b, None);
            str_field(&mut b, "id", id);
            str_field(&mut b, "worldZoneDataId", zone);
            inventory(&mut b, "inventory", &items);
            inventory(&mut b, "craftInventory", &[("coal", 5)]);
            b.push(0x05);
        }
        b.extend([0x07, 0x05]);
        start(&mut b, Some("playerData"));
        inventory(&mut b, "inventory", &[("heal_potion", 6)]);
        b.extend([0x05, 0x05]);

        let got = containers(&b).unwrap();
        assert_eq!(
            got,
            vec![
                Container { object: "player".into(), zone: None, items: BTreeMap::from([("heal_potion".into(), 6)]) },
                // Graves are not storage; fire is not an item; the
                // workbench's craftInventory is not stock.
                Container {
                    object: "chest_rough".into(),
                    zone: Some("mine".into()),
                    items: BTreeMap::from([("coal".into(), 990)]),
                },
            ]
        );
    }
}
