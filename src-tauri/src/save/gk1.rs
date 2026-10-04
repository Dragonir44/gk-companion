//! Graveyard Keeper saves: the game's own "smart serialization".
//!
//! Layout (little-endian):
//! - i64 offset of a string table at the end of the file; i64 + i32 header,
//!   15 reserved i32;
//! - the root object: i32 field count, then per field an i32 hash of its
//!   name (Mono's `string.GetHashCode`) and a typed value;
//! - the string table: i32 count, strings XOR-obfuscated with 0x6D,
//!   referenced by index from `String_Indexed` values.
//!
//! Strings are an i32 *character* count followed by UTF-8 bytes.
//! Format first documented by NetroScript's Graveyard-Keeper-Savefile-Editor (MIT).

use std::collections::{BTreeMap, HashMap};

use super::SaveError;

const HEADER_RESERVED: usize = 15 * 4;
const STRING_XOR: u8 = 0x6D;

/// Root fields kept, mapped to the shared progress list names.
const FIELDS: [(&str, &str); 3] =
    [("unlocked_techs", "unlockedTechs"), ("revealed_techs", "revealedTechs"), ("unlocked_crafts", "unlockedCrafts")];

mod ty {
    pub const NULL: u8 = 0;
    pub const TRUE: u8 = 1;
    pub const FALSE: u8 = 2;
    pub const INT32: u8 = 3;
    pub const INT64: u8 = 4;
    pub const SINGLE: u8 = 5;
    pub const DOUBLE: u8 = 6;
    pub const BYTE: u8 = 7;
    pub const CHAR: u8 = 8;
    pub const STRING: u8 = 9;
    pub const STRING_INDEXED: u8 = 10;
    pub const STRING_EMPTY: u8 = 11;
    pub const JSON: u8 = 12;
    pub const VECTOR2: u8 = 13;
    pub const VECTOR3: u8 = 14;
    pub const QUATERNION: u8 = 15;
    /// 16..=24: constants (0, 1, zero vectors...) with no payload.
    pub const CONSTANTS: std::ops::RangeInclusive<u8> = 16..=24;
    pub const LIST: u8 = 100;
    pub const ARRAY: u8 = 101;
    pub const BYTE_ARRAY: u8 = 102;
    pub const OBJECT: u8 = 250;
}

/// Mono's legacy 32-bit `string.GetHashCode`, used for field names.
pub fn name_hash(s: &str) -> i32 {
    let (mut h1, mut h2) = (5381u32, 5381u32);
    let units: Vec<u16> = s.encode_utf16().collect();
    for pair in units.chunks(2) {
        h1 = (h1 << 5).wrapping_add(h1) ^ pair[0] as u32;
        if let Some(&c) = pair.get(1) {
            h2 = (h2 << 5).wrapping_add(h2) ^ c as u32;
        }
    }
    h1.wrapping_add(h2.wrapping_mul(1566083941)) as i32
}

/// The values the progress needs; everything else is skipped.
enum Value {
    Str(String),
    List(Vec<Value>),
    Other,
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
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
    fn i64(&mut self) -> Result<i64, SaveError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn count(&mut self) -> Result<usize, SaveError> {
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| SaveError::Format(format!("bad count {n} at {}", self.pos - 4)))
    }

    /// `n` characters of UTF-8; `xor` undoes the string table obfuscation,
    /// which only touches single-byte characters other than 0 and 0x6D.
    fn string(&mut self, xor: bool) -> Result<Option<String>, SaveError> {
        let n = self.i32()?;
        if n == -1 {
            return Ok(None);
        }
        let mut out = String::with_capacity(n.max(0) as usize);
        for _ in 0..n.max(0) {
            let b0 = *self.buf.get(self.pos).ok_or(SaveError::Truncated(self.pos))?;
            let len = match b0 {
                0x00..=0x7F => 1,
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                _ => 4,
            };
            let bytes = self.take(len)?;
            if len == 1 {
                let c = if xor && b0 != 0 && b0 != STRING_XOR { b0 ^ STRING_XOR } else { b0 };
                out.push(c as char);
            } else {
                out.push_str(&String::from_utf8_lossy(bytes));
            }
        }
        Ok(Some(out))
    }

    fn value(&mut self, table: &[String]) -> Result<Value, SaveError> {
        let t = self.u8()?;
        Ok(match t {
            ty::OBJECT => {
                self.object(table, &mut |_, _| {})?;
                Value::Other
            }
            ty::STRING | ty::JSON => self.string(false)?.map_or(Value::Other, Value::Str),
            ty::STRING_INDEXED => {
                let i = self.count()?;
                Value::Str(table.get(i).cloned().ok_or_else(|| SaveError::Format(format!("string index {i}")))?)
            }
            ty::STRING_EMPTY => Value::Str(String::new()),
            ty::LIST | ty::ARRAY => {
                let n = self.count()?;
                Value::List((0..n).map(|_| self.value(table)).collect::<Result<_, _>>()?)
            }
            ty::BYTE_ARRAY => {
                let n = self.count()?;
                self.take(n)?;
                Value::Other
            }
            ty::NULL | ty::TRUE | ty::FALSE => Value::Other,
            ty::BYTE | ty::CHAR => {
                self.take(1)?;
                Value::Other
            }
            ty::INT32 | ty::SINGLE => {
                self.take(4)?;
                Value::Other
            }
            ty::INT64 | ty::DOUBLE | ty::VECTOR2 => {
                self.take(8)?;
                Value::Other
            }
            ty::VECTOR3 => {
                self.take(12)?;
                Value::Other
            }
            ty::QUATERNION => {
                self.take(16)?;
                Value::Other
            }
            t if ty::CONSTANTS.contains(&t) => Value::Other,
            t => return Err(SaveError::Format(format!("unknown value type {t} at {}", self.pos - 1))),
        })
    }

    /// Reads an object, handing each field (hash, value) to `field`.
    fn object(&mut self, table: &[String], field: &mut dyn FnMut(i32, Value)) -> Result<(), SaveError> {
        let n = self.i32()?;
        for _ in 0..n.max(0) {
            let hash = self.i32()?;
            let v = self.value(table)?;
            field(hash, v);
        }
        Ok(())
    }
}

pub fn progress_lists(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>, SaveError> {
    let mut r = Reader { buf: bytes, pos: 0 };
    let table_at = usize::try_from(r.i64()?).map_err(|_| SaveError::Format("bad string table offset".into()))?;
    let mut t = Reader { buf: bytes, pos: table_at };
    let n = t.count()?;
    let table = (0..n).map(|_| t.string(true).map(Option::unwrap_or_default)).collect::<Result<Vec<_>, _>>()?;

    r.i64()?; // header offset
    r.i32()?; // version
    r.take(HEADER_RESERVED)?;

    let wanted: HashMap<i32, &str> = FIELDS.iter().map(|(field, list)| (name_hash(field), *list)).collect();
    let mut lists = BTreeMap::new();
    r.object(&table, &mut |hash, v| {
        if let (Some(list), Value::List(items)) = (wanted.get(&hash), v) {
            let strings = items.into_iter().filter_map(|i| if let Value::Str(s) = i { Some(s) } else { None });
            lists.insert(list.to_string(), strings.collect());
        }
    })?;
    if r.pos != table_at {
        return Err(SaveError::Format(format!("save data ends at {} but strings start at {table_at}", r.pos)));
    }
    if lists.is_empty() {
        return Err(SaveError::Format("no progress in save".into()));
    }
    Ok(lists)
}

#[cfg(test)]
mod tests {
    use super::name_hash;

    #[test]
    fn hashes_match_the_game() {
        // Values from saves (also listed in NetroScript's editor).
        assert_eq!(name_hash("unlocked_techs"), -1209696557);
        assert_eq!(name_hash("unlocked_crafts"), -1971702989);
        assert_eq!(name_hash("revealed_techs"), 1300937796);
        assert_eq!(name_hash("known_npcs"), 773086370);
    }
}
