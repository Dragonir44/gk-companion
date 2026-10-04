//! Minimal reader for Unity SerializedFile (`.assets`) metadata.
//!
//! Only what the extractor needs: the type table (with per-type layout
//! hashes) and the object table, so objects can be located and their raw
//! bytes read. Supports format version 22 (Unity 2020.1+), which covers both
//! games. Typetree-enabled files are rejected: neither game ships typetrees.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::UnityError;

pub const CLASS_MONOBEHAVIOUR: i32 = 114;

#[derive(Debug, Clone)]
pub struct SerializedType {
    pub class_id: i32,
    /// Hash of the type's serialized layout; changes when fields change.
    pub type_hash: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub path_id: i64,
    pub byte_start: u64,
    pub byte_size: u32,
    pub type_index: usize,
}

pub struct SerializedFile {
    path: PathBuf,
    pub unity_version: String,
    pub types: Vec<SerializedType>,
    pub objects: Vec<ObjectInfo>,
    data_offset: u64,
}

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

impl SerializedFile {
    pub fn open(path: &Path) -> Result<Self, UnityError> {
        let mut f = File::open(path)?;
        let mut header = [0u8; 48];
        f.read_exact(&mut header)?;
        let be_u32 = |o: usize| u32::from_be_bytes(header[o..o + 4].try_into().unwrap());
        let be_u64 = |o: usize| u64::from_be_bytes(header[o..o + 8].try_into().unwrap());
        let version = be_u32(8);
        if version < 22 {
            return Err(UnityError::Unsupported(format!("serialized file version {version}")));
        }
        let little = header[16] == 0;
        let metadata_size = be_u32(20) as usize;
        let data_offset = be_u64(32);

        // Metadata follows the 48-byte header; read header + metadata so the
        // cursor offsets match file offsets (alignment is file-relative).
        let mut buf = vec![0u8; 48 + metadata_size];
        f.seek(SeekFrom::Start(0))?;
        f.read_exact(&mut buf)?;
        let mut c = Cursor { buf: &buf, pos: 48, little };

        let unity_version = c.cstring()?;
        let _platform = c.i32()?;
        let enable_type_tree = c.u8()? != 0;
        if enable_type_tree {
            return Err(UnityError::Unsupported("files with embedded typetrees".into()));
        }

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
            types.push(SerializedType { class_id, type_hash });
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

        Ok(Self { path: path.to_path_buf(), unity_version, types, objects, data_offset })
    }

    pub fn class_id(&self, obj: &ObjectInfo) -> i32 {
        self.types[obj.type_index].class_id
    }

    pub fn read_object(&self, obj: &ObjectInfo) -> Result<Vec<u8>, UnityError> {
        self.read_at(obj, 0, obj.byte_size as usize)
    }

    fn read_at(&self, obj: &ObjectInfo, offset: usize, len: usize) -> Result<Vec<u8>, UnityError> {
        let len = len.min((obj.byte_size as usize).saturating_sub(offset));
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(self.data_offset + obj.byte_start + offset as u64))?;
        let mut buf = vec![0u8; len];
        f.read_exact(&mut buf)?;
        Ok(buf)
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
