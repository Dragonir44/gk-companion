//! Reader for UnityFS asset bundles (format 6+), as used by Addressables.
//!
//! A bundle is a directory of named nodes (serialized files, `.resS`
//! streams) stored in a stream of compressed blocks. Only LZ4/LZ4HC and
//! uncompressed blocks are supported: that is what the games use.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::UnityError;

const FLAG_COMPRESSION_MASK: u32 = 0x3f;
const FLAG_BLOCKS_INFO_AT_END: u32 = 0x80;
const FLAG_PADDING_BEFORE_DATA: u32 = 0x200;

#[derive(Debug, Clone)]
struct Block {
    uncompressed: u32,
    compressed: u32,
    flags: u16,
}

#[derive(Debug, Clone)]
pub struct BundleNode {
    pub offset: u64,
    pub size: u64,
    pub path: String,
}

pub struct Bundle {
    raw: Vec<u8>,
    data_start: usize,
    blocks: Vec<Block>,
    pub nodes: Vec<BundleNode>,
}

fn decompress(flags: u32, input: &[u8], size: usize) -> Result<Vec<u8>, UnityError> {
    match flags & FLAG_COMPRESSION_MASK {
        0 => Ok(input.to_vec()),
        2 | 3 => lz4_flex::block::decompress(input, size).map_err(|e| UnityError::Format(format!("lz4: {e}"))),
        other => Err(UnityError::Unsupported(format!("bundle compression {other}"))),
    }
}

struct Be<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Be<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], UnityError> {
        let s = self
            .buf
            .get(self.pos..self.pos + n)
            .ok_or_else(|| UnityError::Format("truncated bundle".into()))?;
        self.pos += n;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, UnityError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, UnityError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, UnityError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn cstring(&mut self) -> Result<String, UnityError> {
        let rest = &self.buf[self.pos..];
        let len = rest.iter().position(|&b| b == 0).ok_or_else(|| UnityError::Format("unterminated string".into()))?;
        self.pos += len + 1;
        Ok(String::from_utf8_lossy(&rest[..len]).into_owned())
    }
}

impl Bundle {
    pub fn open(path: &Path) -> Result<Self, UnityError> {
        let mut raw = Vec::new();
        File::open(path)?.read_to_end(&mut raw)?;
        Self::parse(raw)
    }

    /// Parses the header and directory; `raw` may be only the start of the
    /// file (enough for the directory and the first blocks).
    pub fn parse(raw: Vec<u8>) -> Result<Self, UnityError> {
        let mut r = Be { buf: &raw, pos: 0 };
        if r.cstring()? != "UnityFS" {
            return Err(UnityError::Format("not a UnityFS bundle".into()));
        }
        let version = r.u32()?;
        let _player_version = r.cstring()?;
        let _engine_version = r.cstring()?;
        let _size = r.u64()?;
        let info_compressed = r.u32()? as usize;
        let info_size = r.u32()? as usize;
        let flags = r.u32()?;
        if version >= 7 {
            r.pos = (r.pos + 15) & !15;
        }
        if flags & FLAG_BLOCKS_INFO_AT_END != 0 {
            return Err(UnityError::Unsupported("bundle with directory at end".into()));
        }
        let info = decompress(flags, r.take(info_compressed)?, info_size)?;
        let mut data_start = r.pos;
        if flags & FLAG_PADDING_BEFORE_DATA != 0 {
            data_start = (data_start + 15) & !15;
        }

        let mut i = Be { buf: &info, pos: 16 }; // skip content hash
        let block_count = i.u32()? as usize;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            blocks.push(Block { uncompressed: i.u32()?, compressed: i.u32()?, flags: i.u16()? });
        }
        let node_count = i.u32()? as usize;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let offset = i.u64()?;
            let size = i.u64()?;
            let _flags = i.u32()?;
            nodes.push(BundleNode { offset, size, path: i.cstring()? });
        }
        Ok(Self { raw, data_start, blocks, nodes })
    }

    /// Decompresses the data stream up to `len` bytes (or all of it).
    fn stream(&self, len: Option<u64>) -> Result<Vec<u8>, UnityError> {
        let mut out = Vec::new();
        let mut pos = self.data_start;
        for b in &self.blocks {
            if len.is_some_and(|l| out.len() as u64 >= l) {
                break;
            }
            let input = self
                .raw
                .get(pos..pos + b.compressed as usize)
                .ok_or_else(|| UnityError::Format("truncated bundle data".into()))?;
            out.extend(decompress(b.flags as u32, input, b.uncompressed as usize)?);
            pos += b.compressed as usize;
        }
        Ok(out)
    }

    /// Contents of a node; `prefix` limits decompression to its first bytes.
    pub fn read_node(&self, node: &BundleNode, prefix: Option<u64>) -> Result<Vec<u8>, UnityError> {
        let want = prefix.map_or(node.size, |p| p.min(node.size));
        let data = self.stream(Some(node.offset + want))?;
        let end = (node.offset + want) as usize;
        data.get(node.offset as usize..end)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| UnityError::Format(format!("node {} outside data", node.path)))
    }

    pub fn node(&self, path: &str) -> Option<&BundleNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// The serialized file node (the one that isn't a `.resS`/`.resource` stream).
    pub fn serialized_node(&self) -> Option<&BundleNode> {
        self.nodes.iter().find(|n| !n.path.ends_with(".resS") && !n.path.ends_with(".resource"))
    }
}
