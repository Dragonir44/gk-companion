//! Typetree schemas and a reader that decodes raw object bytes into JSON.
//!
//! Schemas come from `tools/schema-gen` (see its docstring). Nodes are stored
//! hash-consed as `[name, type, meta_flag, [child indices]]`.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::UnityError;

const ALIGN_BYTES: u32 = 0x4000;
const MAX_ARRAY_LEN: i32 = 10_000_000;

#[derive(Debug, Deserialize)]
pub struct Node(pub String, pub String, pub u32, pub Vec<usize>);

#[derive(Debug, Deserialize)]
pub struct ClassSchema {
    pub root: usize,
    pub type_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct Schema {
    pub game: String,
    pub unity_version: String,
    pub classes: HashMap<String, ClassSchema>,
    pub nodes: Vec<Node>,
}

impl Schema {
    pub fn parse(json: &str) -> Result<Self, UnityError> {
        let schema: Schema = serde_json::from_str(json).map_err(|e| UnityError::Format(format!("schema: {e}")))?;
        if schema.nodes.iter().flat_map(|n| &n.3).any(|&c| c >= schema.nodes.len()) {
            return Err(UnityError::Format("schema: dangling child index".into()));
        }
        Ok(schema)
    }

    /// Decode `data` with the class `class`. The read must consume the
    /// object exactly: a short or long read means the layout is wrong.
    pub fn decode(&self, class: &str, data: &[u8]) -> Result<Value, UnityError> {
        let cls = self
            .classes
            .get(class)
            .ok_or_else(|| UnityError::Format(format!("schema has no class {class}")))?;
        let mut r = Reader { schema: self, data, pos: 0 };
        let value = r.read(cls.root, class)?;
        if r.pos != data.len() {
            return Err(UnityError::LayoutMismatch(format!(
                "{class}: read {} of {} bytes",
                r.pos,
                data.len()
            )));
        }
        Ok(value)
    }
}

struct Reader<'a> {
    schema: &'a Schema,
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize, path: &str) -> Result<&[u8], UnityError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(|| {
            UnityError::LayoutMismatch(format!("read past end at {path} (offset {})", self.pos))
        })?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn align(&mut self) {
        self.pos = (self.pos + 3) & !3;
    }

    fn i32(&mut self, path: &str) -> Result<i32, UnityError> {
        Ok(i32::from_le_bytes(self.take(4, path)?.try_into().unwrap()))
    }

    fn node(&self, idx: usize) -> &'a Node {
        &self.schema.nodes[idx]
    }

    /// A node is a string when it is the primitive `string` type: either a
    /// bare leaf, or a node whose Array holds `char`. The generator also
    /// labels `List<string>` as `string`, but its Array holds `string`.
    fn is_string(&self, n: &Node) -> bool {
        if n.1 != "string" {
            return false;
        }
        match n.3.first().map(|&a| self.node(a)) {
            None => true,
            Some(arr) => arr.3.get(1).map(|&d| self.node(d).1 == "char").unwrap_or(true),
        }
    }

    fn read(&mut self, idx: usize, path: &str) -> Result<Value, UnityError> {
        let n = self.node(idx);
        let (ty, flags, children) = (n.1.as_str(), n.2, &n.3);

        let value = match ty {
            "SInt8" => Value::from(self.take(1, path)?[0] as i8),
            "UInt8" | "char" => Value::from(self.take(1, path)?[0]),
            "bool" => Value::from(self.take(1, path)?[0] != 0),
            "SInt16" | "short" => Value::from(i16::from_le_bytes(self.take(2, path)?.try_into().unwrap())),
            "UInt16" | "unsigned short" => Value::from(u16::from_le_bytes(self.take(2, path)?.try_into().unwrap())),
            "SInt32" | "int" => Value::from(self.i32(path)?),
            "UInt32" | "unsigned int" | "Type*" => {
                Value::from(u32::from_le_bytes(self.take(4, path)?.try_into().unwrap()))
            }
            "SInt64" | "long long" => Value::from(i64::from_le_bytes(self.take(8, path)?.try_into().unwrap())),
            "UInt64" | "unsigned long long" | "FileSize" => {
                Value::from(u64::from_le_bytes(self.take(8, path)?.try_into().unwrap()))
            }
            "float" => float(f32::from_le_bytes(self.take(4, path)?.try_into().unwrap()) as f64),
            "double" => float(f64::from_le_bytes(self.take(8, path)?.try_into().unwrap())),
            _ if self.is_string(n) => {
                let len = self.i32(path)?;
                if len < 0 {
                    return Err(UnityError::LayoutMismatch(format!("negative string length at {path}")));
                }
                let s = String::from_utf8_lossy(self.take(len as usize, path)?).into_owned();
                self.align();
                Value::String(s)
            }
            "Array" => {
                let len = self.i32(path)?;
                if !(0..=MAX_ARRAY_LEN).contains(&len) {
                    return Err(UnityError::LayoutMismatch(format!("bad array length {len} at {path}")));
                }
                let elem = *children.get(1).ok_or_else(|| UnityError::Format(format!("array without element at {path}")))?;
                let et = self.node(elem).1.as_str();
                if et == "UInt8" || et == "char" {
                    // Raw byte blobs are never needed: skip them.
                    self.take(len as usize, path)?;
                    Value::Null
                } else {
                    let mut items = Vec::with_capacity(len as usize);
                    for i in 0..len {
                        items.push(self.read(elem, &format!("{path}[{i}]"))?);
                    }
                    Value::Array(items)
                }
            }
            _ if children.first().map(|&c| self.node(c).1 == "Array").unwrap_or(false) => {
                self.read(children[0], path)?
            }
            _ => {
                let mut obj = Map::with_capacity(children.len());
                for c in children {
                    let name = self.node(*c).0.clone();
                    let v = self.read(*c, &format!("{path}.{name}"))?;
                    obj.insert(name, v);
                }
                Value::Object(obj)
            }
        };
        if flags & ALIGN_BYTES != 0 {
            self.align();
        }
        Ok(value)
    }
}

fn float(f: f64) -> Value {
    serde_json::Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null)
}
