//! Item/building icons from the games' sprite atlases.
//!
//! gk1 keeps its `icons` atlas in resources.assets (no typetrees: layouts
//! come from the schema's built-in classes). gk2 keeps `Icons` and
//! `IconsCompressed` in Addressables bundles (with typetrees), found by
//! scanning bundle metadata since bundle file names are content hashes.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use super::ExtractError;
use crate::model::GameId;
use crate::unity::bundle::{Bundle, BundleNode};
use crate::unity::serialized::{self, ObjectInfo, SerializedFile, SerializedType, CLASS_SPRITE, CLASS_SPRITE_ATLAS, CLASS_TEXTURE2D};
use crate::unity::typetree::{self, Schema};
use crate::unity::UnityError;

/// Atlases holding inventory icons, compared case-insensitively.
const ICON_ATLASES: [&str; 2] = ["icons", "iconscompressed"];
const FORMAT_RGBA32: i64 = 4;
const FORMAT_ARGB32: i64 = 5;
/// Icon outlines are drawn in pure blue and recoloured by the games' UI
/// shader; replaced here by a dark outline close to the in-game look.
const OUTLINE_KEY: [u8; 3] = [0, 0, 255];
const OUTLINE: [u8; 3] = [34, 26, 30];
/// Bundle scan threads; more mostly contend for the disk.
const MAX_SCAN_THREADS: usize = 8;
/// Enough of a bundle's start for its directory and first block.
const BUNDLE_PEEK: usize = 256 * 1024;

/// A decoded texture, rows top to bottom, RGBA8.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Sprite position in a sheet, top-left origin.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub sheet: usize,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Default)]
pub struct IconSet {
    pub sheets: Vec<Sheet>,
    pub sprites: HashMap<String, Rect>,
    /// The world map, its image being one of the sheets (gk2).
    pub map: Option<crate::model::WorldMap>,
    pub map_error: Option<String>,
}

impl IconSet {
    pub fn has(&self, name: &str) -> bool {
        self.sprites.contains_key(name)
    }

    /// Writes sheets as `<prefix>-<n>.png` into `dir`; returns file names.
    pub fn write_sheets(&self, dir: &Path, prefix: &str) -> Result<Vec<String>, ExtractError> {
        std::fs::create_dir_all(dir)?;
        let mut names = Vec::new();
        for (i, s) in self.sheets.iter().enumerate() {
            let name = format!("{prefix}-{i}.png");
            let file = std::io::BufWriter::new(std::fs::File::create(dir.join(&name))?);
            let mut enc = png::Encoder::new(file, s.width, s.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| ExtractError::Icons(e.to_string()))?;
            w.write_image_data(&s.rgba).map_err(|e| ExtractError::Icons(e.to_string()))?;
            names.push(name);
        }
        Ok(names)
    }
}

/// Where a texture's pixels live when streamed out of the object.
pub(super) enum StreamSource<'a> {
    /// gk1: `.resS` files next to the assets.
    Dir(&'a Path),
    /// gk2: `.resS` nodes of the same bundle.
    Bundle(&'a Bundle),
}

pub(super) struct Ctx<'a> {
    pub file: &'a SerializedFile,
    pub schema: &'a Schema,
    pub streams: StreamSource<'a>,
}

impl Ctx<'_> {
    pub fn decode(&self, obj: &ObjectInfo) -> Result<Value, ExtractError> {
        let raw = self.file.read_object(obj)?;
        let class = self.file.class_id(obj);
        let what = format!("class {class} object {}", obj.path_id);
        let v = match self.file.tree(obj) {
            Some(tree) => typetree::decode(tree, 0, &raw, &what)?,
            None => {
                let root = self
                    .schema
                    .builtin_root(class)
                    .ok_or_else(|| ExtractError::Missing(format!("layout of class {class}")))?;
                typetree::decode(&self.schema.nodes, root, &raw, &what)?
            }
        };
        Ok(v)
    }

    pub fn object(&self, pptr: &Value) -> Result<&ObjectInfo, ExtractError> {
        if pptr.get("m_FileID").and_then(Value::as_i64) != Some(0) {
            return Err(ExtractError::Icons("reference to another file".into()));
        }
        let id = pptr.get("m_PathID").and_then(Value::as_i64).unwrap_or(0);
        self.file.object(id).ok_or_else(|| ExtractError::Icons(format!("missing object {id}")))
    }

    fn stream(&self, path: &str, offset: u64, size: u64) -> Result<Vec<u8>, ExtractError> {
        match &self.streams {
            StreamSource::Dir(dir) => {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(dir.join(path))?;
                f.seek(SeekFrom::Start(offset))?;
                let mut buf = vec![0u8; size as usize];
                f.read_exact(&mut buf)?;
                Ok(buf)
            }
            StreamSource::Bundle(b) => {
                let name = path.rsplit('/').next().unwrap_or(path);
                let node = b.node(name).ok_or_else(|| ExtractError::Icons(format!("missing stream {name}")))?;
                let data = b.read_node(node, Some(offset + size))?;
                Ok(data[offset as usize..].to_vec())
            }
        }
    }

    /// Decodes a texture; `icon` applies the icon outline colour.
    pub fn texture(&self, obj: &ObjectInfo, icon: bool) -> Result<Sheet, ExtractError> {
        let t = self.decode(obj)?;
        let width = t.get("m_Width").and_then(Value::as_u64).unwrap_or(0) as u32;
        let height = t.get("m_Height").and_then(Value::as_u64).unwrap_or(0) as u32;
        let format = t.get("m_TextureFormat").and_then(Value::as_i64).unwrap_or(-1);
        let sd = t.get("m_StreamData").cloned().unwrap_or_default();
        let size = sd.get("size").and_then(Value::as_u64).unwrap_or(0);
        if size == 0 {
            return Err(ExtractError::Icons("texture data not streamed".into()));
        }
        let offset = sd.get("offset").and_then(Value::as_u64).unwrap_or(0);
        let raw = self.stream(sd.get("path").and_then(Value::as_str).unwrap_or_default(), offset, size)?;
        let row = width as usize * 4;
        if raw.len() < row * height as usize {
            return Err(ExtractError::Icons("texture data too short".into()));
        }
        // Unity stores rows bottom-up.
        let mut rgba = Vec::with_capacity(row * height as usize);
        for y in (0..height as usize).rev() {
            let line = &raw[y * row..(y + 1) * row];
            match format {
                FORMAT_RGBA32 => rgba.extend_from_slice(line),
                FORMAT_ARGB32 => line.chunks_exact(4).for_each(|p| rgba.extend_from_slice(&[p[1], p[2], p[3], p[0]])),
                f => return Err(ExtractError::Icons(format!("texture format {f}"))),
            }
        }
        for px in rgba.chunks_exact_mut(4) {
            if px[3] == 0 {
                // Leftover colour in transparent pixels bleeds into edges
                // when scaled: clear it.
                px.copy_from_slice(&[0, 0, 0, 0]);
            } else if icon && px[..3] == OUTLINE_KEY {
                px[..3].copy_from_slice(&OUTLINE);
            }
        }
        Ok(Sheet { width, height, rgba })
    }

    /// Adds an atlas's sprites to `set` if it is an icon atlas.
    fn add_atlas(&self, obj: &ObjectInfo, set: &mut IconSet) -> Result<(), ExtractError> {
        let atlas = self.decode(obj)?;
        let name = atlas.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_lowercase();
        if !ICON_ATLASES.contains(&name.as_str()) {
            return Ok(());
        }
        let render: Vec<(&Value, &Value)> = list(&atlas, "m_RenderDataMap")
            .iter()
            .filter_map(|p| Some((p.get("first")?, p.get("second")?)))
            .collect();
        let names = list(&atlas, "m_PackedSpriteNamesToIndex");
        let mut sheet_of: HashMap<i64, usize> = HashMap::new();

        for (pptr, sprite_name) in list(&atlas, "m_PackedSprites").iter().zip(names) {
            let Some(sprite_name) = sprite_name.as_str() else { continue };
            let sprite = self.decode(self.object(pptr)?)?;
            let key = sprite.get("m_RenderDataKey");
            let Some((_, data)) = render.iter().find(|(k, _)| Some(*k) == key) else { continue };
            let tex_ref = data.get("texture").cloned().unwrap_or_default();
            let tex_id = tex_ref.get("m_PathID").and_then(Value::as_i64).unwrap_or(0);
            let sheet = match sheet_of.get(&tex_id) {
                Some(&s) => s,
                None => {
                    set.sheets.push(self.texture(self.object(&tex_ref)?, true)?);
                    sheet_of.insert(tex_id, set.sheets.len() - 1);
                    set.sheets.len() - 1
                }
            };
            let r = data.get("textureRect").cloned().unwrap_or_default();
            let f = |k: &str| r.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            let (w, h) = (f("width").round() as u32, f("height").round() as u32);
            let sheet_h = set.sheets[sheet].height;
            let y = sheet_h.saturating_sub(f("y").round() as u32 + h);
            set.sprites.insert(sprite_name.to_string(), Rect { sheet, x: f("x").round() as u32, y, w, h });
        }
        Ok(())
    }
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub fn load(game: GameId, data: &Path, schema: &Schema) -> Result<IconSet, ExtractError> {
    let mut set = IconSet::default();
    match game {
        GameId::Gk1 => {
            let file = SerializedFile::open(&data.join("resources.assets"))?;
            let ctx = Ctx { file: &file, schema, streams: StreamSource::Dir(data) };
            for obj in file.objects_of(CLASS_SPRITE_ATLAS) {
                ctx.add_atlas(obj, &mut set)?;
            }
        }
        GameId::Gk2 => {
            let scan = scan_bundles(data, schema)?;
            for path in &scan.atlases {
                let bundle = Bundle::open(path)?;
                let node = bundle.serialized_node().ok_or_else(|| ExtractError::Icons("bundle without file".into()))?;
                let bytes = Arc::new(bundle.read_node(node, None)?);
                let file = SerializedFile::from_bytes(bytes, Some(&schema.common_strings))?;
                let ctx = Ctx { file: &file, schema, streams: StreamSource::Bundle(&bundle) };
                for obj in file.objects_of(CLASS_SPRITE_ATLAS) {
                    ctx.add_atlas(obj, &mut set)?;
                }
            }
            if set.sprites.is_empty() {
                return Err(ExtractError::Icons("no icon atlas found".into()));
            }
            match super::map::load(&scan, schema) {
                Ok((sheet, map)) => {
                    let (w, h) = (sheet.width, sheet.height);
                    set.sheets.push(sheet);
                    set.sprites.insert(map.sprite.clone(), Rect { sheet: set.sheets.len() - 1, x: 0, y: 0, w, h });
                    set.map = Some(map);
                }
                Err(e) => set.map_error = Some(e.to_string()),
            }
        }
    }
    if set.sprites.is_empty() {
        return Err(ExtractError::Icons("no icon atlas found".into()));
    }
    Ok(set)
}

/// What a scan of the Addressables bundles found (gk2).
#[derive(Default)]
pub struct BundleScan {
    /// Bundles holding a sprite atlas.
    pub atlases: Vec<PathBuf>,
    /// Bundles of the map window prefabs (have `worldZonePoints`).
    pub map_pages: Vec<PathBuf>,
    /// Bundle of the main scene, and its scene file's node.
    pub main_scene: Option<(PathBuf, String)>,
    /// Bundle by the name of its serialized file (`CAB-<hash>`), to follow
    /// references between bundles.
    pub by_cab: HashMap<String, PathBuf>,
}

/// Interesting parts of one bundle, from its directory and metadata.
#[derive(Default)]
struct Peek {
    atlas: bool,
    map_page: bool,
    /// Scene file node, when it is the main scene.
    main_scene: Option<String>,
    cab: Option<String>,
}

/// Field of the map window's behaviour, unique to it.
const MAP_PAGE_FIELD: &str = "worldZonePoints";
/// Field of the main scene's UI root (world bounds of the map).
const MAIN_SCENE_FIELD: &str = "worldMin";
/// Scene bundles hold `CAB-<hash>` (the scene) and `CAB-<hash>.sharedAssets`.
const SHARED_ASSETS: &str = ".sharedAssets";

/// Scans the Addressables bundles (gk2) for atlases and the map's parts.
pub fn scan_bundles(data: &Path, schema: &Schema) -> Result<BundleScan, ExtractError> {
    let aa = data.join("StreamingAssets/aa");
    let platform = std::fs::read_dir(&aa)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "AddressablesLink"))
        .ok_or_else(|| ExtractError::Missing("Addressables bundles".into()))?;
    let mut bundles: Vec<PathBuf> = std::fs::read_dir(&platform)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "bundle"))
        .collect();
    bundles.sort();
    // Tens of thousands of small reads: spread them over a few threads.
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(MAX_SCAN_THREADS);
    let chunk = bundles.len().div_ceil(workers).max(1);
    let peeks: Vec<(PathBuf, Peek)> = std::thread::scope(|scope| {
        let handles: Vec<_> = bundles
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|p| (p.clone(), peek(p, schema).unwrap_or_default())).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut scan = BundleScan::default();
    for (path, p) in peeks {
        if p.atlas {
            scan.atlases.push(path.clone());
        }
        if p.map_page {
            scan.map_pages.push(path.clone());
        }
        if let Some(node) = p.main_scene {
            scan.main_scene = Some((path.clone(), node));
        }
        if let Some(cab) = p.cab {
            scan.by_cab.insert(cab, path);
        }
    }
    Ok(scan)
}

/// Reads a bundle's directory and type table without decompressing it all.
fn peek(path: &Path, schema: &Schema) -> Result<Peek, UnityError> {
    use std::io::Read;
    let mut head = Vec::with_capacity(BUNDLE_PEEK);
    std::fs::File::open(path)?.take(BUNDLE_PEEK as u64).read_to_end(&mut head)?;
    let read = |raw: Vec<u8>| -> Result<Peek, UnityError> {
        let b = Bundle::parse(raw)?;
        let types_of = |node: &BundleNode| -> Result<Vec<SerializedType>, UnityError> {
            let start = b.read_node(node, Some(48))?;
            let meta = b.read_node(node, Some(serialized::metadata_len(&start)? as u64))?;
            serialized::peek_types(&meta, Some(&schema.common_strings))
        };
        let has_field = |types: &[SerializedType], field: &str| {
            types.iter().any(|t| t.tree.as_ref().is_some_and(|tree| tree.iter().any(|n| n.0 == field)))
        };
        // A scene bundle: the scene file is the shared assets' twin.
        let scene = b.nodes.iter().find_map(|n| b.node(n.path.strip_suffix(SHARED_ASSETS)?));
        if let Some(scene) = scene {
            let main = has_field(&types_of(scene)?, MAIN_SCENE_FIELD).then(|| scene.path.clone());
            return Ok(Peek { main_scene: main, cab: Some(scene.path.clone()), ..Default::default() });
        }
        let Some(node) = b.serialized_node() else { return Ok(Peek::default()) };
        let types = types_of(node)?;
        let has = |c: i32| types.iter().any(|t| t.class_id == c);
        let atlas = has(CLASS_SPRITE_ATLAS) && has(CLASS_SPRITE) && has(CLASS_TEXTURE2D);
        let map_page = has_field(&types, MAP_PAGE_FIELD);
        Ok(Peek { atlas, map_page, main_scene: None, cab: Some(node.path.clone()) })
    };
    match read(head) {
        Ok(v) => Ok(v),
        // Directory or metadata beyond the peeked bytes: read it all.
        Err(_) => read(std::fs::read(path)?),
    }
}

/// Sprites referenced by entities, positioned in their sheets.
pub fn referenced(set: &IconSet, names: impl Iterator<Item = String>) -> BTreeMap<String, [u32; 5]> {
    names
        .filter_map(|n| set.sprites.get(&n).map(|r| (n, [r.sheet as u32, r.x, r.y, r.w, r.h])))
        .collect()
}
