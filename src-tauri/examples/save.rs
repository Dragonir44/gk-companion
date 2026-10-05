//! Reads progress from a save: `cargo run --example save -- gk1|gk2 <save.dat> [out.json]`
//! (`out.json`: slot + progress, for the browser dev mode's mock)
use gk_companion_lib::{model::GameId, save};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = if args.get(1).map(String::as_str) == Some("gk1") { GameId::Gk1 } else { GameId::Gk2 };
    let path = std::path::PathBuf::from(args.get(2).expect("usage: save gk1|gk2 <file.dat> [out.json]"));
    let slot = save::list_slots(&[path.parent().unwrap().to_path_buf()])
        .into_iter()
        .find(|s| s.path == path)
        .expect("not a save slot");
    let t = std::time::Instant::now();
    let p = save::read_progress(game, &slot).unwrap_or_else(|e| panic!("{e}"));
    println!("read {} in {:?}, info: {:?}", p.slot, t.elapsed(), slot.info.map(|i| i["day"].clone()));
    for (k, v) in &p.lists {
        println!("{k}: {} {:?}", v.len(), &v[..v.len().min(3)]);
    }
    for c in &p.inventories {
        let total: u32 = c.items.values().sum();
        println!("stock {:<28} {:<22} {} items, {} kinds", c.object, c.zone.as_deref().unwrap_or("-"), total, c.items.len());
    }
    if let Some(out) = args.get(3) {
        let slot = save::list_slots(&[path.parent().unwrap().to_path_buf()]).into_iter().find(|s| s.path == path).unwrap();
        let json = serde_json::json!({ "slot": slot, "progress": p });
        std::fs::write(out, serde_json::to_string(&json).unwrap()).unwrap();
        println!("wrote {out}");
    }
}
