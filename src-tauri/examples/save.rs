//! Reads progress from a save: `cargo run --example save -- <save.dat> [out.json]`
//! (`out.json`: slot + progress, for the browser dev mode's mock)
use gk_companion_lib::{model::GameId, save};

fn main() {
    let path = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: save <file.dat>"));
    let slot = save::list_slots(&[path.parent().unwrap().to_path_buf()])
        .into_iter()
        .find(|s| s.path == path)
        .expect("not a save slot");
    let t = std::time::Instant::now();
    let p = save::read_progress(GameId::Gk2, &slot).unwrap_or_else(|e| panic!("{e}"));
    println!("read {} in {:?}, info: {:?}", p.slot, t.elapsed(), slot.info.map(|i| i["day"].clone()));
    for (k, v) in &p.lists {
        println!("{k}: {} {:?}", v.len(), &v[..v.len().min(3)]);
    }
    if let Some(out) = std::env::args().nth(2) {
        let slot = save::list_slots(&[path.parent().unwrap().to_path_buf()]).into_iter().find(|s| s.path == path).unwrap();
        let json = serde_json::json!({ "slot": slot, "progress": p });
        std::fs::write(&out, serde_json::to_string(&json).unwrap()).unwrap();
        println!("wrote {out}");
    }
}
