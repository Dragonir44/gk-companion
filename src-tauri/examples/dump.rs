//! Writes a game's raw `GameBalance` as JSON, for investigating game data:
//! `cargo run --release --example dump -- gk1|gk2 "<game dir>" out.json [lang]`
//! With `lang` (fr, en...), also writes that raw text table to `out.<lang>.json`.
use gk_companion_lib::{extract, model::GameId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = match args.get(1).map(String::as_str) {
        Some("gk1") => GameId::Gk1,
        Some("gk2") => GameId::Gk2,
        _ => panic!("usage: dump gk1|gk2 <game dir> <out.json>"),
    };
    let raw = extract::raw_balance(game, std::path::Path::new(&args[2])).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&args[3], serde_json::to_string(&raw).unwrap()).unwrap();
    println!("wrote {}", args[3]);
    if let Some(lang) = args.get(4) {
        let texts = extract::raw_locale(game, std::path::Path::new(&args[2]), lang).unwrap_or_else(|e| panic!("{e}"));
        let out = args[3].trim_end_matches(".json").to_string() + &format!(".{lang}.json");
        std::fs::write(&out, serde_json::to_string(&texts).unwrap()).unwrap();
        println!("wrote {out}");
    }
}
