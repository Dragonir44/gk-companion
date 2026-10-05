//! Minimal reader for Unity SerializedFile (`.assets` files and the
//! serialized files inside asset bundles).
//!
//! Reads the type table (layout hashes, and embedded typetrees when the file
//! has them) and the object table, so objects can be located and their raw
//! bytes read. Supports format version 22 (Unity 2020.1+): both games.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::typetree::Node;
use super::UnityError;

pub const CLASS_GAMEOBJECT: i32 = 1;
pub const CLASS_TRANSFORM: i32 = 4;
pub const CLASS_MONOBEHAVIOUR: i32 = 114;
pub const CLASS_TEXTURE2D: i32 = 28;
pub const CLASS_SPRITE: i32 = 213;
pub const CLASS_SPRITE_ATLAS: i32 = 687078895;

const HEADER_SIZE: usize = 48;

#[derive(Debug, Clone)]
pub struct SerializedType {
    pub class_id: i32,
    /// Hash of the type's serialized layout; changes when fields change.
    pub type_hash: [u8; 16],
    /// Embedded typetree (root at index 0), when the file has them.
    pub tree: Option<Arc<Vec<Node>>>,
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub path_id: i64,
    pub byte_start: u64,
    pub byte_size: u32,
    pub type_index: usize,
}

enum Backing {
    File(PathBuf),
    /// Whole file in memory (serialized files extracted from bundles).
    Memory(Arc<Vec<u8>>),
}

pub struct SerializedFile {
    backing: Backing,
    pub unity_version: String,
    pub types: Vec<SerializedType>,
    pub objects: Vec<ObjectInfo>,
    /// Other files this one references: a PPtr's `m_FileID` n is
    /// `externals[n - 1]` (bundles: `archive:/CAB-<hash>/CAB-<hash>`).
    pub externals: Vec<String>,
    data_offset: u64,
}

/// Unity's shared typetree strings, by offset (see tools/schema-gen).
pub type CommonStrings = HashMap<u32, String>;

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    little: bool,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], UnityError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len());
        let end = end.ok_or_else(|| UnityError::Format(format!("unexpected end of metadata at {}", self.pos)))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], UnityError> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        if !self.little {
            a.reverse();
        }
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, UnityError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, UnityError> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    fn i16(&mut self) -> Result<i16, UnityError> {
        Ok(i16::from_le_bytes(self.arr()?))
    }
    fn i32(&mut self) -> Result<i32, UnityError> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    fn u32(&mut self) -> Result<u32, UnityError> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    fn i64(&mut self) -> Result<i64, UnityError> {
        Ok(i64::from_le_bytes(self.arr()?))
    }
    fn bytes16(&mut self) -> Result<[u8; 16], UnityError> {
        let mut a = [0u8; 16];
        a.copy_from_slice(self.take(16)?);
        Ok(a)
    }
    fn cstring(&mut self) -> Result<String, UnityError> {
        let rest = &self.buf[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| UnityError::Format("unterminated string".into()))?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(s)
    }
    /// Alignment is relative to the start of the file, which is where the
    /// cursor's buffer starts.
    fn align4(&mut self) {
        self.pos = (self.pos + 3) & !3;
    }
}

struct Header {
    little: bool,
    metadata_size: usize,
    data_offset: u64,
}

fn parse_header(buf: &[u8]) -> Result<Header, UnityError> {
    if buf.len() < HEADER_SIZE {
        return Err(UnityError::Format("file too short".into()));
    }
    let be_u32 = |o: usize| u32::from_be_bytes(buf[o..o + 4].try_into().unwrap());
    let be_u64 = |o: usize| u64::from_be_bytes(buf[o..o + 8].try_into().unwrap());
    let version = be_u32(8);
    if version < 22 {
        return Err(UnityError::Unsupported(format!("serialized file version {version}")));
    }
    Ok(Header { little: buf[16] == 0, metadata_size: be_u32(20) as usize, data_offset: be_u64(32) })
}

/// Bytes needed from the start of a file to parse its metadata.
pub fn metadata_len(head: &[u8]) -> Result<usize, UnityError> {
    Ok(HEADER_SIZE + parse_header(head)?.metadata_size)
}

/// Embedded typetree blob: flat nodes with depth levels, plus a string
/// buffer; offsets with the high bit set point into Unity's common strings.
fn parse_tree(c: &mut Cursor, common: Option<&CommonStrings>) -> Result<Vec<Node>, UnityError> {
    let count = c.i32()?.max(0) as usize;
    let strings_len = c.i32()?.max(0) as usize;
    let mut flat = Vec::with_capacity(count);
    for _ in 0..count {
        let _version = c.u16()?;
        let level = c.u8()?;
        let _type_flags = c.u8()?;
        let type_off = c.u32()?;
        let name_off = c.u32()?;
        let _byte_size = c.i32()?;
        let _index = c.i32()?;
        let meta = c.i32()? as u32;
        let _ref_type_hash = c.take(8)?;
        flat.push((level, type_off, name_off, meta));
    }
    let strings = c.take(strings_len)?;
    let string_at = |off: u32| -> Result<String, UnityError> {
        if off & 0x8000_0000 != 0 {
            let common = common.ok_or_else(|| UnityError::Format("typetree needs common strings".into()))?;
            return common
                .get(&(off & 0x7fff_ffff))
                .cloned()
                .ok_or_else(|| UnityError::Format(format!("unknown common string {off:#x}")));
        }
        let rest = strings.get(off as usize..).ok_or_else(|| UnityError::Format("bad string offset".into()))?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    };

    let mut nodes: Vec<Node> = Vec::with_capacity(count);
    // Stack of (level, node index) of the current ancestors.
    let mut stack: Vec<(u8, usize)> = Vec::new();
    for (level, type_off, name_off, meta) in flat {
        let idx = nodes.len();
        nodes.push(Node(string_at(name_off)?, string_at(type_off)?, meta, vec![]));
        while stack.last().is_some_and(|&(l, _)| l >= level) {
            stack.pop();
        }
        if let Some(&(_, parent)) = stack.last() {
            nodes[parent].3.push(idx);
        }
        stack.push((level, idx));
    }
    Ok(nodes)
}

struct Metadata {
    unity_version: String,
    types: Vec<SerializedType>,
    objects: Vec<ObjectInfo>,
    externals: Vec<String>,
}

fn parse_metadata(buf: &[u8], common: Option<&CommonStrings>) -> Result<Metadata, UnityError> {
    let h = parse_header(buf)?;
    let mut c = Cursor { buf, pos: HEADER_SIZE, little: h.little };

    let unity_version = c.cstring()?;
    let _platform = c.i32()?;
    let enable_type_tree = c.u8()? != 0;

    let type_count = c.i32()?;
    let mut types = Vec::with_capacity(type_count.max(0) as usize);
    for _ in 0..type_count {
        let class_id = c.i32()?;
        let _stripped = c.u8()?;
        let _script_type_index = c.i16()?;
        if class_id == CLASS_MONOBEHAVIOUR {
            let _script_id = c.bytes16()?;
        }
        let type_hash = c.bytes16()?;
        let tree = if enable_type_tree {
            let t = parse_tree(&mut c, common)?;
            let deps = c.i32()?.max(0) as usize;
            c.take(deps * 4)?;
            Some(Arc::new(t))
        } else {
            None
        };
        types.push(SerializedType { class_id, type_hash, tree });
    }

    let object_count = c.i32()?;
    let mut objects = Vec::with_capacity(object_count.max(0) as usize);
    for _ in 0..object_count {
        c.align4();
        let path_id = c.i64()?;
        let byte_start = c.i64()? as u64;
        let byte_size = c.u32()?;
        let type_index = c.i32()? as usize;
        if type_index >= types.len() {
            return Err(UnityError::Format(format!("object {path_id} has bad type index")));
        }
        objects.push(ObjectInfo { path_id, byte_start, byte_size, type_index });
    }

    // Script references (file index, aligned local id), then externals.
    let script_count = c.i32()?;
    for _ in 0..script_count {
        c.i32()?;
        c.align4();
        c.i64()?;
    }
    let external_count = c.i32()?;
    let mut externals = Vec::with_capacity(external_count.max(0) as usize);
    for _ in 0..external_count {
        let _temp_empty = c.cstring()?;
        let _guid = c.bytes16()?;
        let _kind = c.i32()?;
        externals.push(c.cstring()?);
    }
    Ok(Metadata { unity_version, types, objects, externals })
}

/// Class ids of the types in a file, from its metadata alone. Lets bundle
/// scans skip files without decompressing them whole.
pub fn peek_types(head: &[u8], common: Option<&CommonStrings>) -> Result<Vec<SerializedType>, UnityError> {
    Ok(parse_metadata(head, common)?.types)
}

impl SerializedFile {
    pub fn open(path: &Path) -> Result<Self, UnityError> {
        let mut f = File::open(path)?;
        let mut head = [0u8; HEADER_SIZE];
        f.read_exact(&mut head)?;
        let mut buf = vec![0u8; metadata_len(&head)?];
        f.seek(SeekFrom::Start(0))?;
        f.read_exact(&mut buf)?;
        let m = parse_metadata(&buf, None)?;
        let data_offset = parse_header(&head)?.data_offset;
        Ok(Self {
            backing: Backing::File(path.to_path_buf()),
            unity_version: m.unity_version,
            types: m.types,
            objects: m.objects,
            externals: m.externals,
            data_offset,
        })
    }

    pub fn from_bytes(data: Arc<Vec<u8>>, common: Option<&CommonStrings>) -> Result<Self, UnityError> {
        let m = parse_metadata(&data, common)?;
        let data_offset = parse_header(&data)?.data_offset;
        Ok(Self {
            backing: Backing::Memory(data),
            unity_version: m.unity_version,
            types: m.types,
            objects: m.objects,
            externals: m.externals,
            data_offset,
        })
    }

    pub fn class_id(&self, obj: &ObjectInfo) -> i32 {
        self.types[obj.type_index].class_id
    }

    pub fn tree(&self, obj: &ObjectInfo) -> Option<&Arc<Vec<Node>>> {
        self.types[obj.type_index].tree.as_ref()
    }

    /// Whether the object's embedded typetree has a field named `field`.
    pub fn has_field(&self, obj: &ObjectInfo, field: &str) -> bool {
        self.tree(obj).is_some_and(|t| t.iter().any(|n| n.0 == field))
    }

    pub fn object(&self, path_id: i64) -> Option<&ObjectInfo> {
        self.objects.iter().find(|o| o.path_id == path_id)
    }

    pub fn objects_of(&self, class_id: i32) -> impl Iterator<Item = &ObjectInfo> {
        self.objects.iter().filter(move |o| self.class_id(o) == class_id)
    }

    pub fn read_object(&self, obj: &ObjectInfo) -> Result<Vec<u8>, UnityError> {
        self.read_at(obj, 0, obj.byte_size as usize)
    }

    fn read_at(&self, obj: &ObjectInfo, offset: usize, len: usize) -> Result<Vec<u8>, UnityError> {
        let len = len.min((obj.byte_size as usize).saturating_sub(offset));
        let start = self.data_offset + obj.byte_start + offset as u64;
        match &self.backing {
            Backing::File(path) => {
                let mut f = File::open(path)?;
                f.seek(SeekFrom::Start(start))?;
                let mut buf = vec![0u8; len];
                f.read_exact(&mut buf)?;
                Ok(buf)
            }
            Backing::Memory(data) => data
                .get(start as usize..start as usize + len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| UnityError::Format("object outside file".into())),
        }
    }

    /// Name of a MonoBehaviour: m_GameObject (PPtr, 12) + m_Enabled (u8,
    /// aligned to 16) + m_Script (PPtr, 12) put m_Name at offset 28.
    pub fn monobehaviour_name(&self, obj: &ObjectInfo) -> Result<String, UnityError> {
        let head = self.read_at(obj, 28, 4 + 256)?;
        if head.len() < 4 {
            return Ok(String::new());
        }
        let len = i32::from_le_bytes(head[..4].try_into().unwrap());
        if len < 0 || len as usize > head.len() - 4 {
            return Ok(String::new());
        }
        Ok(String::from_utf8_lossy(&head[4..4 + len as usize]).into_owned())
    }

    /// Find MonoBehaviours by name. Small objects are skipped, which keeps
    /// the scan cheap on files with tens of thousands of behaviours.
    pub fn find_monobehaviours(
        &self,
        wanted: impl Fn(&str) -> bool,
        min_size: u32,
    ) -> Result<Vec<(String, ObjectInfo)>, UnityError> {
        let mut found = Vec::new();
        for obj in &self.objects {
            if self.class_id(obj) != CLASS_MONOBEHAVIOUR || obj.byte_size < min_size {
                continue;
            }
            let name = self.monobehaviour_name(obj)?;
            if wanted(&name) {
                found.push((name, obj.clone()));
            }
        }
        Ok(found)
    }

    pub fn type_hash_hex(&self, obj: &ObjectInfo) -> String {
        self.types[obj.type_index].type_hash.iter().map(|b| format!("{b:02x}")).collect()
    }
}
