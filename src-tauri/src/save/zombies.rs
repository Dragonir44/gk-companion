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
//! carousel, the barracks).

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
    studied: Vec<String>,
    points: BTreeMap<String, i64>,
}

pub fn zombies(bytes: &[u8]) -> Result<Vec<Zombie>, SaveError> {
    let mut r = Reader::new(bytes);
    // Frame names: the node's name, "" when unnamed, `ARRAY` for arrays.
    let mut stack: Vec<String> = Vec::new();
    let mut wgo: Option<Wgo> = None;
    let mut found: Vec<Wgo> = Vec::new();
    let mut objects: HashMap<String, String> = HashMap::new();
    // Body `uniqueId` -> id of the place holding it.
    let mut held: HashMap<String, String> = HashMap::new();
    loop {
        let entry = r.next_entry()?;
        // Path below the object's node (owned: the stack changes below).
        let path: Vec<String> = wgo.as_ref().map(|w| stack[w.depth + 1..].to_vec()).unwrap_or_default();
        let rel: Vec<&str> = path.iter().map(String::as_str).collect();
        match entry {
            Entry::Start(name, _) => {
                let n = stack.len();
                if name.is_none() && n >= 2 && stack[n - 1] == ARRAY && stack[n - 2] == "wgoDataList" {
                    wgo = Some(Wgo { depth: n, ..Default::default() });
                }
                stack.push(name.unwrap_or_default());
            }
            Entry::StartArray => stack.push(ARRAY.into()),
            Entry::Str(field, value) => {
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
                    for body in &w.holds {
                        held.insert(body.clone(), w.id.clone());
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
    Ok(found
        .into_iter()
        .map(|w| Zombie {
            name: w.name.clone().unwrap_or_default(),
            station: w
                .attached
                .as_ref()
                .and_then(|g| objects.get(g))
                .or_else(|| w.body.as_ref().and_then(|b| held.get(b)))
                .cloned(),
            tool: equipped(&w.items, &w.hand),
            collar: equipped(&w.items, &w.collar),
            items: w.items.into_iter().map(|(id, _)| id).collect(),
            object: w.id,
            zone: w.zone,
            studied: w.studied,
            points: w.points,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::testbin::*;
    use super::*;

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
        start(&mut b, Some("mainWgoPartData"));
        start(&mut b, Some("dockPointDataList"));
        arr(&mut b);
        start(&mut b, None);
        guid(&mut b, "occupiedBy", "body-4");
        b.extend([0x05, 0x07, 0x05, 0x05, 0x05]);
        b.extend([0x07, 0x05, 0x05]);

        let got = zombies(&b).unwrap();
        assert_eq!(got[1].station.as_deref(), Some("zombie_carousel"));
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
                studied: vec!["z_building_lvl_up_1_1".into(), "ztalent_logger".into()],
                points: BTreeMap::from([("r".into(), 120)]),
            }]
        );
    }
}
