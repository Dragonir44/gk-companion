//! The gk2 world map, as the game's map window draws it.
//!
//! The window prefab (`MapPageWidget`) holds the map image (`mapRect`'s
//! sprite, in another bundle) and fixed points for interiors
//! (`worldZonePoints`). The main scene's `GUIElements` holds two markers,
//! `worldMin` and `worldMax`: the game places a world position on the map
//! by interpolating linearly between them.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use super::icons::{BundleScan, Ctx, Sheet, StreamSource};
use super::ExtractError;
use crate::model::WorldMap;
use crate::unity::bundle::Bundle;
use crate::unity::serialized::{
    SerializedFile, CLASS_GAMEOBJECT, CLASS_MONOBEHAVIOUR, CLASS_TRANSFORM,
};
use crate::unity::typetree::Schema;

/// Sprite name of the map image in the icon index.
pub const MAP_SPRITE: &str = "ui:world_map";

fn err(msg: impl Into<String>) -> ExtractError {
    ExtractError::Missing(format!("world map: {}", msg.into()))
}

fn f(v: &Value, path: &[&str]) -> Option<f64> {
    path.iter().try_fold(v, |v, k| v.get(k))?.as_f64()
}

fn path_id(pptr: &Value) -> i64 {
    pptr.get("m_PathID").and_then(Value::as_i64).unwrap_or(0)
}

fn file_id(pptr: &Value) -> i64 {
    pptr.get("m_FileID").and_then(Value::as_i64).unwrap_or(0)
}

/// A bundle's serialized file, read whole.
fn open(path: &std::path::Path, schema: &Schema) -> Result<(Bundle, SerializedFile), ExtractError> {
    let bundle = Bundle::open(path)?;
    let node = bundle
        .serialized_node()
        .ok_or_else(|| err("bundle without file"))?;
    let bytes = Arc::new(bundle.read_node(node, None)?);
    let file = SerializedFile::from_bytes(bytes, Some(&schema.common_strings))?;
    Ok((bundle, file))
}

pub fn load(scan: &BundleScan, schema: &Schema) -> Result<(Sheet, WorldMap), ExtractError> {
    let (world_min, world_max) = world_rect(scan, schema)?;
    // Big and small window variants: only some reference their image
    // directly (others load it at run time).
    let mut last = err("map window not found");
    for page in &scan.map_pages {
        match from_page(scan, schema, page) {
            Ok((image, zones)) => {
                return Ok((
                    image,
                    WorldMap {
                        sprite: MAP_SPRITE.into(),
                        world_min,
                        world_max,
                        zones,
                    },
                ))
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

type Points = BTreeMap<String, [f32; 2]>;

/// The map image and interior points of one map window prefab.
fn from_page(
    scan: &BundleScan,
    schema: &Schema,
    page_path: &std::path::Path,
) -> Result<(Sheet, Points), ExtractError> {
    let (page_bundle, page_file) = open(page_path, schema)?;
    let ctx = Ctx {
        file: &page_file,
        schema,
        streams: StreamSource::Bundle(&page_bundle),
    };
    let page_obj = page_file
        .objects_of(CLASS_MONOBEHAVIOUR)
        .find(|o| page_file.has_field(o, "worldZonePoints"))
        .ok_or_else(|| err("map window behaviour not found"))?;
    let page = ctx.decode(page_obj)?;

    // The map's rectangle, and the image on its game object.
    let rect = ctx.decode(ctx.object(page.get("mapRect").ok_or_else(|| err("no mapRect"))?)?)?;
    let size = [
        f(&rect, &["m_SizeDelta", "x"]).unwrap_or(0.0),
        f(&rect, &["m_SizeDelta", "y"]).unwrap_or(0.0),
    ];
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return Err(err("empty map rectangle"));
    }
    let go = ctx.decode(
        ctx.object(
            rect.get("m_GameObject")
                .ok_or_else(|| err("no game object"))?,
        )?,
    )?;
    let sprite_ref = go
        .get("m_Component")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("component"))
        .filter(|p| file_id(p) == 0)
        .filter_map(|p| page_file.object(path_id(p)))
        .filter(|o| page_file.class_id(o) == CLASS_MONOBEHAVIOUR)
        .filter_map(|o| ctx.decode(o).ok()?.get("m_Sprite").cloned())
        .find(|s| path_id(s) != 0)
        .ok_or_else(|| err("map image not found"))?;
    let image = sprite_image(scan, schema, &page_file, &sprite_ref)?;

    // Interior points: anchored from the rectangle's centre, y up.
    let zones = page
        .get("worldZonePoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = p.get("worldZoneId")?.as_str()?.to_string();
            let (x, y) = (f(p, &["mapPosition", "x"])?, f(p, &["mapPosition", "y"])?);
            Some((id, [(0.5 + x / size[0]) as f32, (0.5 - y / size[1]) as f32]))
        })
        .collect::<Points>();
    Ok((image, zones))
}

/// The map image: a sprite in another bundle, cropped from its texture.
fn sprite_image(
    scan: &BundleScan,
    schema: &Schema,
    from: &SerializedFile,
    sprite: &Value,
) -> Result<Sheet, ExtractError> {
    let external = usize::try_from(file_id(sprite) - 1)
        .ok()
        .and_then(|i| from.externals.get(i))
        .ok_or_else(|| err("bad image reference"))?;
    let cab = external.rsplit('/').next().unwrap_or(external);
    let path = scan
        .by_cab
        .get(cab)
        .ok_or_else(|| err(format!("image bundle {cab} not found")))?;
    let (bundle, file) = open(path, schema)?;
    let ctx = Ctx {
        file: &file,
        schema,
        streams: StreamSource::Bundle(&bundle),
    };
    let obj = file
        .object(path_id(sprite))
        .ok_or_else(|| err("image sprite not found"))?;
    let s = ctx.decode(obj)?;
    let texture = s
        .get("m_RD")
        .and_then(|rd| rd.get("texture"))
        .ok_or_else(|| err("sprite without texture"))?;
    let sheet = ctx.texture(ctx.object(texture)?, false)?;
    // The sprite's rectangle in its texture (bottom-left origin).
    let r = |k: &str| f(&s, &["m_Rect", k]).unwrap_or(0.0).round() as u32;
    let (w, h) = (r("width").min(sheet.width), r("height").min(sheet.height));
    if w == 0 || h == 0 || (w, h) == (sheet.width, sheet.height) {
        return Ok(sheet);
    }
    let (x, y) = (r("x"), sheet.height.saturating_sub(r("y") + h));
    let row = sheet.width as usize * 4;
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for line in y..y + h {
        let start = line as usize * row + x as usize * 4;
        rgba.extend_from_slice(&sheet.rgba[start..start + w as usize * 4]);
    }
    Ok(Sheet {
        width: w,
        height: h,
        rgba,
    })
}

/// World positions of the main scene's `worldMin`/`worldMax` markers.
fn world_rect(scan: &BundleScan, schema: &Schema) -> Result<([f32; 2], [f32; 2]), ExtractError> {
    let (path, node) = scan
        .main_scene
        .as_ref()
        .ok_or_else(|| err("main scene not found"))?;
    let bundle = Bundle::open(path)?;
    let node = bundle
        .node(node)
        .ok_or_else(|| err("main scene file not found"))?;
    let file = SerializedFile::from_bytes(
        Arc::new(bundle.read_node(node, None)?),
        Some(&schema.common_strings),
    )?;
    let ctx = Ctx {
        file: &file,
        schema,
        streams: StreamSource::Bundle(&bundle),
    };
    let gui = file
        .objects_of(CLASS_MONOBEHAVIOUR)
        .find(|o| file.has_field(o, "worldMin") && file.has_field(o, "worldMax"))
        .ok_or_else(|| err("world bounds not found"))?;
    let gui = ctx.decode(gui)?;
    let position = |key: &str| -> Result<[f32; 2], ExtractError> {
        // Sum local positions up the hierarchy (the markers are not rotated).
        let mut pptr = gui
            .get(key)
            .cloned()
            .ok_or_else(|| err(format!("no {key}")))?;
        let (mut x, mut z) = (0.0, 0.0);
        let mut depth = 0;
        while path_id(&pptr) != 0 && depth < 32 {
            let obj = ctx.object(&pptr)?;
            if file.class_id(obj) != CLASS_TRANSFORM && file.class_id(obj) != CLASS_GAMEOBJECT {
                return Err(err(format!("{key} is not a transform")));
            }
            let t = ctx.decode(obj)?;
            x += f(&t, &["m_LocalPosition", "x"]).unwrap_or(0.0);
            z += f(&t, &["m_LocalPosition", "z"]).unwrap_or(0.0);
            pptr = t.get("m_Father").cloned().unwrap_or_default();
            depth += 1;
        }
        Ok([x as f32, z as f32])
    };
    let (a, b) = (position("worldMin")?, position("worldMax")?);
    if a[0] == b[0] || a[1] == b[1] {
        return Err(err("degenerate world bounds"));
    }
    // The game takes min/max per axis, whichever marker holds them.
    Ok((
        [a[0].min(b[0]), a[1].min(b[1])],
        [a[0].max(b[0]), a[1].max(b[1])],
    ))
}
