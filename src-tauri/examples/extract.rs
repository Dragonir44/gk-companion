//! Runs an extraction outside the app: `cargo run --example extract -- gk1 <game dir> [out.json]`
use gk_companion_lib::{extract, model::GameId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = match args.get(1).map(String::as_str) {
        Some("gk1") => GameId::Gk1,
        Some("gk2") => GameId::Gk2,
        _ => panic!("usage: extract gk1|gk2 <game dir> [out.json]"),
    };
    let t = std::time::Instant::now();
    let data = extract::extract(game, std::path::Path::new(&args[2])).unwrap_or_else(|e| panic!("{e}"));
    println!("extracted in {:?}", t.elapsed());
    let named = |es: &[gk_companion_lib::model::Entity]| es.iter().filter(|e| e.name.is_some()).count();
    println!("items {} (named {})", data.items.len(), named(&data.items));
    println!("objects {} (named {})", data.objects.len(), named(&data.objects));
    println!("recipes {} techs {} (named {}) groups {}", data.recipes.len(), data.techs.len(),
        data.techs.iter().filter(|t| t.name.is_some()).count(), data.groups.len());
    for (lang, t) in &data.locales { print!("{lang}:{} ", t.len()); }
    println!();
    if let Some(out) = args.get(3) {
        std::fs::write(out, serde_json::to_string(&data).unwrap()).unwrap();
        println!("wrote {out} ({} bytes)", std::fs::metadata(out).unwrap().len());
    }
}
