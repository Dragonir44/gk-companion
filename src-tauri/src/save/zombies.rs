//! Zombies of a gk2 save: their body, equipment and talents.
//!
//! A zombie is a world object (`wgoDataList/*`) with a `zombieItem`: the
//! body, whose `inventory` holds its organs, collar and tools. `equipped*`
//! fields point at those items by `uniqueId`. `talentData/*/studiedLevelUps`
//! lists the tree nodes bought; `techRed/Green/Blue` are the points left.
//!
//! Where it works is told three ways: its `attachedWgoDataUniqueId` (the
//! station's `uniqueId`), or the place pointing at its body's `uniqueId`,
//! as `workerId` (stations) or a dock point's `occupiedBy` (the factory
//! carousel, the barracks). Each barracks slot is a squad, told apart by
//! its own `uniqueId` (`place`).
//!
//! `militaryBaseData/baseBuildings` lists the defences built (towers,
//! barricades) by `uniqueId`.
//!
//! A `uniqueId` (an `SGuid` node) is written once; later mentions are
//! references to that node, resolved here to the guid.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::odin::{Entry, Reader};
use super::SaveError;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Zombie {
    /// Text key of its name (`zombie_name_12`).
    pub name: String,
    /// Its world object id (`zombie_worker`).
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// World object id of the station it works at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    /// Everything in the body: organs, extras, embalming, collar, tools.
    pub items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor: Option<String>,
    /// `uniqueId` of the station instance (one barracks slot = one squad).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// Talent tree nodes bought.
    pub studied: Vec<String>,
    /// Points left to spend, keyed r, g, b.
    pub points: BTreeMap<String, i64>,
}

const ARRAY: &str = "[]";
const NO_GUID: &str = "00000000-0000-0000-0000-000000000000";

/// The object being read, its path relative to its own node.
#[derive(Default)]
struct Wgo {
    depth: usize,
    id: String,
    guid: Option<String>,
    zone: Option<String>,
    name: Option<String>,
    zombie: bool,
    /// `uniqueId` of its body (`zombieItem`).
    body: Option<String>,
    /// Bodies it holds as worker or on its dock points.
    holds: Vec<String>,
    /// Body items: (id, uniqueId).
    items: Vec<(String, Option<String>)>,
    item: (Option<String>, Option<String>),
    attached: Option<String>,
    hand: Option<String>,
    collar: Option<String>,
    weapon: Option<String>,
    armor: Option<String>,
    studied: Vec<String>,
    points: BTreeMap<String, i64>,
}

/// Zombies, and the defences built at the military base (object ids).
pub struct Army {
    pub zombies: Vec<Zombie>,
    pub defences: Vec<String>,
}

pub fn army(bytes: &[u8]) -> Result<Army, SaveError> {
    let mut r = Reader::new(bytes);
    // Frame names: the node's name, "" when unnamed, `ARRAY` for arrays.
    let mut stack: Vec<String> = Vec::new();
    let mut wgo: Option<Wgo> = None;
    let mut found: Vec<Wgo> = Vec::new();
    let mut objects: HashMap<String, String> = HashMap::new();
    // Body `uniqueId` -> uniqueId of the place holding it.
    let mut held: HashMap<String, String> = HashMap::new();
    let mut base: Vec<String> = Vec::new();
    // SGuid node id -> guid, and the SGuid node being read (depth, id).
    let mut guids: HashMap<i32, String> = HashMap::new();
    let mut guid_node: Option<(usize, i32)> = None;
    // A resolved reference pushed a frame for one entry.
    let mut pushed = false;
    loop {
        if std::mem::take(&mut pushed) {
            stack.pop();
        }
        let entry = match r.next_entry()? {
            // Stands for the node it points to: `<name> { id = guid }`.
            Entry::Ref(name, id) => match guids.get(&id) {
                Some(g) => {
                    stack.push(name.unwrap_or_default());
                    pushed = true;
                    Entry::Str(Some("id".into()), g.clone())
                }
                None => Entry::Other(name),
            },
            e => e,
        };
        // Path below the object's node (owned: the stack changes below).
        let path: Vec<String> = wgo.as_ref().map(|w| stack[w.depth + 1..].to_vec()).unwrap_or_default();
        let rel: Vec<&str> = path.iter().map(String::as_str).collect();
        match entry {
            Entry::Start(name, ty) => {
                let n = stack.len();
                if name.is_none() && n >= 2 && stack[n - 1] == ARRAY && stack[n - 2] == "wgoDataList" {
                    wgo = Some(Wgo { depth: n, ..Default::default() });
                }
                if ty.is_some_and(|t| t.starts_with("SGuid")) {
                    guid_node = r.node_id.map(|id| (n + 1, id));
                }
                stack.push(name.unwrap_or_default());
            }
            Entry::StartArray => stack.push(ARRAY.into()),
            Entry::Str(field, value) => {
                let n = stack.len();
                if let Some((depth, id)) = guid_node {
                    if depth == n && field.as_deref() == Some("id") && !pushed {
                        guids.insert(id, value.clone());
                        guid_node = None;
                    }
                }
                if field.as_deref() == Some("id")
                    && n >= 4
                    && stack[n - 4..] == ["militaryBaseData", "baseBuildings", ARRAY, ""].map(String::from)
                {
                    base.push(value.clone());
                }
                let Some(w) = &mut wgo else { continue };
                let guid = || Some(value.clone()).filter(|v| v != NO_GUID);
                match (rel.as_slice(), field.as_deref()) {
                    ([], Some("id")) => w.id = value.clone(),
                    ([], Some("worldZoneDataId")) if !value.is_empty() => w.zone = Some(value.clone()),
                    ([], Some("name")) => w.name = Some(value.clone()),
                    (["uniqueId"], Some("id")) => w.guid = guid(),
                    (["attachedWgoDataUniqueId"], Some("id")) => w.attached = guid(),
                    (["zombieItem", "uniqueId"], Some("id")) => w.body = guid(),
                    (["workerId"], Some("id")) | (["mainWgoPartData", "dockPointDataList", ARRAY, "", "occupiedBy"], Some("id")) => {
                        w.holds.extend(guid())
                    }
                    (["equippedHand"], Some("id")) => w.hand = guid(),
                    (["equippedCollar"], Some("id")) => w.collar = guid(),
                    (["equippedWeapon"], Some("id")) => w.weapon = guid(),
                    (["equippedArmor"], Some("id")) => w.armor = guid(),
                    (["zombieItem", "inventory", ARRAY, ""], Some("id")) => w.item.0 = Some(value.clone()),
                    (["zombieItem", "inventory", ARRAY, "", "uniqueId"], Some("id")) => w.item.1 = Some(value.clone()),
                    (["talentData", ARRAY, "", "studiedLevelUps", ARRAY], None) => w.studied.push(value.clone()),
                    _ => {}
                }
            }
            Entry::Int(Some(field), v) => {
                if let (Some(w), []) = (&mut wgo, rel.as_slice()) {
                    let key = match field.as_str() {
                        "techRed" => "r",
                        "techGreen" => "g",
                        "techBlue" => "b",
                        _ => continue,
                    };
                    w.points.insert(key.into(), v);
                }
            }
            Entry::End | Entry::EndArray => {
                let Some(frame) = stack.pop() else { break };
                let Some(w) = &mut wgo else { continue };
                if frame == "zombieItem" && rel.len() == 1 {
                    w.zombie = true;
                } else if frame.is_empty() && rel == ["zombieItem", "inventory", ARRAY, ""] {
                    if let (Some(id), guid) = std::mem::take(&mut w.item) {
                        w.items.push((id, guid));
                    }
                }
                if stack.len() == w.depth {
                    let w = wgo.take().unwrap();
                    if let Some(g) = &w.guid {
                        objects.insert(g.clone(), w.id.clone());
                    }
                    if let Some(g) = &w.guid {
                        for body in &w.holds {
                            held.insert(body.clone(), g.clone());
                        }
                    }
                    if w.zombie {
                        found.push(w);
                    }
                }
            }
            Entry::EndOfStream => break,
            _ => {}
        }
    }
    let equipped = |items: &[(String, Option<String>)], guid: &Option<String>| {
        guid.as_ref().and_then(|g| items.iter().find(|(_, u)| u.as_ref() == Some(g)).map(|(id, _)| id.clone()))
    };
    let zombies = found
        .into_iter()
        .map(|w| {
            let place = w.attached.clone().filter(|g| objects.contains_key(g)).or_else(|| w.body.as_ref().and_then(|b| held.get(b).cloned()));
            Zombie {
                name: w.name.clone().unwrap_or_default(),
                station: place.as_ref().and_then(|g| objects.get(g)).cloned(),
                tool: equipped(&w.items, &w.hand),
                collar: equipped(&w.items, &w.collar),
                weapon: equipped(&w.items, &w.weapon),
                armor: equipped(&w.items, &w.armor),
                place,
                items: w.items.into_iter().map(|(id, _)| id).collect(),
                object: w.id,
                zone: w.zone,
                studied: w.studied,
                points: w.points,
            }
        })
        .collect();
    let defences = base.iter().filter_map(|g| objects.get(g).cloned()).collect();
    Ok(Army { zombies, defences })
}

#[cfg(test)]
mod tests {
    use super::super::testbin::*;
    use super::*;

    /// Named reference node (0x01) of type `SGuid` with its node id.
    fn guid_node(b: &mut Vec<u8>, name: &str, node: i32, id: &str) {
        b.push(0x01);
        b.extend(s8(name));
        b.push(0x2F);
        b.extend(0i32.to_le_bytes());
        b.extend(s8("SGuid, Assembly-CSharp"));
        b.extend(node.to_le_bytes());
        str_field(b, "id", id);
        b.push(0x05);
    }

    #[test]
    fn resolves_guid_references() {
        let mut b = Vec::new();
        start(&mut b, None);
        start(&mut b, Some("wgoDataList"));
        arr(&mut b);
        start(&mut b, None);
        str_field(&mut b, "id", "tower_2_pre");
        guid_node(&mut b, "uniqueId", 7, "t-1");
        b.push(0x05);
        b.extend([0x07, 0x05]);
        // The base lists it by reference to node 7 (unnamed internal ref, 0x0A).
        start(&mut b, Some("militaryBaseData"));
        start(&mut b, Some("baseBuildings"));
        arr(&mut b);
        b.push(0x0A);
        b.extend(7i32.to_le_bytes());
        b.extend([0x07, 0x05, 0x05, 0x05]);
        assert_eq!(army(&b).unwrap().defences, vec!["tower_2_pre".to_string()]);
    }

    fn guid(b: &mut Vec<u8>, name: &str, id: &str) {
        start(b, Some(name));
        str_field(b, "id", id);
        b.push(0x05);
    }

    #[test]
    fn reads_body_equipment_and_talents() {
        let mut b = Vec::new();
        start(&mut b, None);
        start(&mut b, Some("wgoDataList"));
        arr(&mut b);
        // The station, then the zombie working at it.
        start(&mut b, None);
        str_field(&mut b, "id", "sawmill");
        guid(&mut b, "uniqueId", "s-1");
        b.push(0x05);

        start(&mut b, None);
        str_field(&mut b, "id", "zombie_worker");
        guid(&mut b, "uniqueId", "z-1");
        str_field(&mut b, "worldZoneDataId", "yard");
        str_field(&mut b, "name", "zombie_name_3");
        start(&mut b, Some("zombieItem"));
        str_field(&mut b, "id", "body_zombie");
        start(&mut b, Some("inventory"));
        arr(&mut b);
        for (id, g) in [("skin_3_0:3", "i-1"), ("axe_2", "i-2"), ("collar_gold", "i-3")] {
            start(&mut b, None);
            str_field(&mut b, "id", id);
            guid(&mut b, "uniqueId", g);
            b.push(0x05);
        }
        b.extend([0x07, 0x05, 0x05]);
        guid(&mut b, "attachedWgoDataUniqueId", "s-1");
        guid(&mut b, "equippedHand", "i-2");
        guid(&mut b, "equippedCollar", "i-3");
        start(&mut b, Some("talentData"));
        arr(&mut b);
        start(&mut b, None);
        str_field(&mut b, "id", "talent_orange");
        start(&mut b, Some("studiedLevelUps"));
        arr(&mut b);
        str_value(&mut b, "z_building_lvl_up_1_1");
        str_value(&mut b, "ztalent_logger");
        b.extend([0x07, 0x05, 0x05, 0x07, 0x05]);
        int_field(&mut b, "techRed", 120);
        b.push(0x05);

        // A zombie with no station of its own, on the carousel's dock point.
        start(&mut b, None);
        str_field(&mut b, "id", "zombie");
        str_field(&mut b, "name", "zombie_name_4");
        start(&mut b, Some("zombieItem"));
        str_field(&mut b, "id", "body_zombie");
        guid(&mut b, "uniqueId", "body-4");
        b.push(0x05);
        b.push(0x05);
        start(&mut b, None);
        str_field(&mut b, "id", "zombie_carousel");
        guid(&mut b, "uniqueId", "c-1");
        start(&mut b, Some("mainWgoPartData"));
        start(&mut b, Some("dockPointDataList"));
        arr(&mut b);
        start(&mut b, None);
        guid(&mut b, "occupiedBy", "body-4");
        b.extend([0x05, 0x07, 0x05, 0x05, 0x05]);
        b.extend([0x07, 0x05, 0x05]);

        let got = army(&b).unwrap().zombies;
        assert_eq!(got[1].station.as_deref(), Some("zombie_carousel"));
        assert_eq!(got[1].place.as_deref(), Some("c-1"));
        assert_eq!(
            got[..1],
            [Zombie {
                name: "zombie_name_3".into(),
                object: "zombie_worker".into(),
                zone: Some("yard".into()),
                station: Some("sawmill".into()),
                items: vec!["skin_3_0:3".into(), "axe_2".into(), "collar_gold".into()],
                tool: Some("axe_2".into()),
                collar: Some("collar_gold".into()),
                weapon: None,
                armor: None,
                place: Some("s-1".into()),
                studied: vec!["z_building_lvl_up_1_1".into(), "ztalent_logger".into()],
                points: BTreeMap::from([("r".into(), 120)]),
            }]
        );
    }
}
