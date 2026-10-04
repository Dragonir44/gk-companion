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
    let extracted = extract::extract(game, std::path::Path::new(&args[2]), None).unwrap_or_else(|e| panic!("{e}"));
    let mut data = extracted.data;
    println!("extracted in {:?}", t.elapsed());
    let named = |es: &[gk_companion_lib::model::Entity]| es.iter().filter(|e| e.name.is_some()).count();
    println!("items {} (named {})", data.items.len(), named(&data.items));
    println!("objects {} (named {})", data.objects.len(), named(&data.objects));
    println!("recipes {} techs {} (named {}) groups {}", data.recipes.len(), data.techs.len(),
        data.techs.iter().filter(|t| t.name.is_some()).count(), data.groups.len());
    for (lang, t) in &data.locales { print!("{lang}:{} ", t.len()); }
    println!();
    let with_icon = |es: &[gk_companion_lib::model::Entity]| es.iter().filter(|e| e.icon.is_some()).count();
    println!(
        "icons: {} sprites used, items {}/{}, objects {}/{}, recipes {}{}",
        data.icons.sprites.len(),
        with_icon(&data.items),
        data.items.len(),
        with_icon(&data.objects),
        data.objects.len(),
        data.recipes.iter().filter(|r| r.icon.is_some()).count(),
        extracted.icons_error.map(|e| format!(" (error: {e})")).unwrap_or_default(),
    );
    if let Some(out) = args.get(3) {
        // Sheets go next to the output file, as in the app's cache dir.
        if let Some(set) = &extracted.icons {
            let dir = std::path::Path::new(out).parent().unwrap_or(std::path::Path::new("."));
            data.icons.sheets = set.write_sheets(dir, &format!("{}-icons", game.as_str())).unwrap();
        }
        std::fs::write(out, serde_json::to_string(&data).unwrap()).unwrap();
        println!("wrote {out} ({} bytes)", std::fs::metadata(out).unwrap().len());
    }
}
