//! Streaming reader for Odin Serializer's binary format (Sirenix), used by
//! gk2 saves. Yields entries one by one; values other than strings are
//! skipped, which is all the save reader needs.
//!
//! Entry codes: odd codes 0x01..=0x2D (except 0x05/0x07) and 0x32 are
//! "named" (a name string follows the code); code + 1 is the unnamed form.

use super::SaveError;

#[derive(Debug, PartialEq)]
pub enum Entry {
    /// Start of a reference or struct node (`name`, type name).
    Start(Option<String>, Option<String>),
    End,
    StartArray,
    EndArray,
    Str(Option<String>, String),
    /// Integer values (int, uint, long, ulong).
    Int(Option<String>, i64),
    /// Any other value, skipped.
    Other(Option<String>),
    EndOfStream,
}

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    /// Type ids defined earlier in the stream (`TypeName` entries).
    types: Vec<Option<String>>,
}

const NAMED_START_REF: u8 = 0x01;
const NAMED_START_STRUCT: u8 = 0x03;
const END_OF_NODE: u8 = 0x05;
const START_ARRAY: u8 = 0x06;
const END_ARRAY: u8 = 0x07;
const PRIMITIVE_ARRAY: u8 = 0x08;
const TYPE_NAME: u8 = 0x2F;
const TYPE_ID: u8 = 0x30;
const END_OF_STREAM: u8 = 0x31;
const NAMED_NULL: u8 = 0x2D;
const NAMED_EXTERNAL_BY_STRING: u8 = 0x32;

fn is_named(code: u8) -> bool {
    (code % 2 == 1 && (NAMED_START_REF..=NAMED_NULL).contains(&code) && code != END_OF_NODE && code != END_ARRAY)
        || code == NAMED_EXTERNAL_BY_STRING
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0, types: Vec::new() }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        let s = self.buf.get(self.pos..self.pos + n).ok_or(SaveError::Truncated(self.pos))?;
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, SaveError> {
        Ok(self.take(1)?[0])
    }

    fn i32(&mut self) -> Result<i32, SaveError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// Strings: a char-size flag (0: 8-bit, 1: UTF-16), a length, the chars.
    fn string(&mut self) -> Result<String, SaveError> {
        let wide = self.u8()? != 0;
        let len = self.i32()?;
        if len < 0 {
            return Err(SaveError::Format(format!("negative string length at {}", self.pos)));
        }
        let len = len as usize;
        if wide {
            let raw = self.take(len * 2)?;
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            Ok(String::from_utf16_lossy(&units))
        } else {
            Ok(self.take(len)?.iter().map(|&b| b as char).collect())
        }
    }

    fn type_entry(&mut self) -> Result<Option<String>, SaveError> {
        match self.u8()? {
            TYPE_NAME => {
                let id = self.i32()?.max(0) as usize;
                let name = self.string()?;
                if self.types.len() <= id {
                    self.types.resize(id + 1, None);
                }
                self.types[id] = Some(name.clone());
                Ok(Some(name))
            }
            TYPE_ID => {
                let id = self.i32()?.max(0) as usize;
                Ok(self.types.get(id).cloned().flatten())
            }
            0x2E => Ok(None), // unnamed null: no type
            t => Err(SaveError::Format(format!("bad type entry {t:#x} at {}", self.pos - 1))),
        }
    }

    pub fn next_entry(&mut self) -> Result<Entry, SaveError> {
        if self.pos >= self.buf.len() {
            return Ok(Entry::EndOfStream);
        }
        let code = self.u8()?;
        let named = is_named(code);
        let name = if named { Some(self.string()?) } else { None };
        // Unnamed variants share their named code's layout.
        let base = if code > 0 && is_named(code - 1) { code - 1 } else { code };
        Ok(match base {
            NAMED_START_REF => {
                let ty = self.type_entry()?;
                self.i32()?; // reference id
                Entry::Start(name, ty)
            }
            NAMED_START_STRUCT => Entry::Start(name, self.type_entry()?),
            END_OF_NODE => Entry::End,
            START_ARRAY => {
                self.take(8)?; // element count
                Entry::StartArray
            }
            END_ARRAY => Entry::EndArray,
            PRIMITIVE_ARRAY => {
                let count = self.i32()?.max(0) as usize;
                let size = self.i32()?.max(0) as usize;
                self.take(count * size)?;
                Entry::Other(name)
            }
            0x09 | 0x0B => {
                self.take(4)?; // internal reference / external by index
                Entry::Other(name)
            }
            0x0D | 0x29 | 0x23 => {
                self.take(16)?; // guid, decimal
                Entry::Other(name)
            }
            0x0F | 0x11 | 0x2B => {
                self.take(1)?;
                Entry::Other(name)
            }
            0x13 | 0x15 | 0x25 => {
                self.take(2)?;
                Entry::Other(name)
            }
            0x17 => Entry::Int(name, i32::from_le_bytes(self.take(4)?.try_into().unwrap()) as i64),
            0x19 => Entry::Int(name, u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as i64),
            0x1B => Entry::Int(name, i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            0x1D => Entry::Int(name, u64::from_le_bytes(self.take(8)?.try_into().unwrap()) as i64),
            0x1F => {
                self.take(4)?;
                Entry::Other(name)
            }
            0x21 => {
                self.take(8)?;
                Entry::Other(name)
            }
            0x27 => {
                let s = self.string()?;
                Entry::Str(name, s)
            }
            NAMED_NULL => Entry::Other(name),
            NAMED_EXTERNAL_BY_STRING => {
                self.string()?;
                Entry::Other(name)
            }
            END_OF_STREAM => Entry::EndOfStream,
            other => return Err(SaveError::Format(format!("unknown entry {other:#x} at {}", self.pos - 1))),
        })
    }
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

    fn s16(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut v = vec![1u8];
        v.extend((units.len() as i32).to_le_bytes());
        units.iter().for_each(|u| v.extend(u.to_le_bytes()));
        v
    }

    #[test]
    fn reads_nodes_arrays_and_strings() {
        let mut b = vec![0x02, TYPE_NAME];
        b.extend(0i32.to_le_bytes());
        b.extend(s16("GameSave"));
        b.extend(0i32.to_le_bytes()); // ref id
        b.push(0x17); // named int
        b.extend(s8("day"));
        b.extend(189i32.to_le_bytes());
        b.push(0x01); // named ref
        b.extend(s8("list"));
        b.push(TYPE_ID);
        b.extend(0i32.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.push(START_ARRAY);
        b.extend(1i64.to_le_bytes());
        b.push(0x28); // unnamed string
        b.extend(s16("wood_basic"));
        b.extend([END_ARRAY, END_OF_NODE, END_OF_NODE]);

        let mut r = Reader::new(&b);
        let mut got = Vec::new();
        loop {
            let e = r.next_entry().unwrap();
            if e == Entry::EndOfStream {
                break;
            }
            got.push(e);
        }
        assert_eq!(
            got,
            vec![
                Entry::Start(None, Some("GameSave".into())),
                Entry::Int(Some("day".into()), 189),
                Entry::Start(Some("list".into()), Some("GameSave".into())),
                Entry::StartArray,
                Entry::Str(None, "wood_basic".into()),
                Entry::EndArray,
                Entry::End,
                Entry::End,
            ]
        );
    }
}
