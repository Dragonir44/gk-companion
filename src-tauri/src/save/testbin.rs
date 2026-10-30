//! Builders for small Odin binary streams in tests.

pub fn s8(s: &str) -> Vec<u8> {
    let mut v = vec![0u8];
    v.extend((s.len() as i32).to_le_bytes());
    v.extend(s.as_bytes());
    v
}
/// Named struct start (0x03) with no type (0x2E).
pub fn start(b: &mut Vec<u8>, name: Option<&str>) {
    match name {
        Some(n) => {
            b.push(0x03);
            b.extend(s8(n));
        }
        None => b.push(0x04),
    }
    b.push(0x2E);
}
pub fn arr(b: &mut Vec<u8>) {
    b.push(0x06);
    b.extend(0i64.to_le_bytes());
}
pub fn str_field(b: &mut Vec<u8>, name: &str, v: &str) {
    b.push(0x27);
    b.extend(s8(name));
    b.extend(s8(v));
}
pub fn item(b: &mut Vec<u8>, id: &str, count: i32) {
    start(b, None);
    str_field(b, "id", id);
    b.push(0x17);
    b.extend(s8("count"));
    b.extend(count.to_le_bytes());
    b.push(0x05);
}
pub fn int_field(b: &mut Vec<u8>, name: &str, v: i32) {
    b.push(0x17);
    b.extend(s8(name));
    b.extend(v.to_le_bytes());
}

/// Unnamed string (list element).
pub fn str_value(b: &mut Vec<u8>, v: &str) {
    b.push(0x28);
    b.extend(s8(v));
}
