//! Prints a gk2 save as an indented tree, for investigating save fields:
//! `cargo run --release --example odin_tree -- <save.dat> > tree.txt`
use gk_companion_lib::save::odin::{Entry, Reader};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let buf = std::fs::read(&a[1]).unwrap();
    let mut r = Reader::new(&buf);
    let mut d = 0usize;
    loop {
        let e = match r.next_entry() { Ok(e) => e, Err(e) => { println!("ERR {e:?}"); break } };
        let p = "  ".repeat(d);
        match e {
            Entry::Start(n, t) => { println!("{p}{} <{}>", n.unwrap_or_default(), t.unwrap_or_default()); d += 1 }
            Entry::End => d = d.saturating_sub(1),
            Entry::StartArray => { println!("{p}["); d += 1 }
            Entry::EndArray => { d = d.saturating_sub(1); println!("{p}]") }
            Entry::Str(n, s) => println!("{p}{} = {s:?}", n.unwrap_or_default()),
            Entry::Int(n, v) => println!("{p}{} = {v}", n.unwrap_or_default()),
            Entry::Float(n, v) => println!("{p}{} = {v}", n.unwrap_or_default()),
            Entry::Other(n) => println!("{p}{} = ?", n.unwrap_or_default()),
            Entry::EndOfStream => break,
        }
    }
}
