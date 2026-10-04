# GK Companion

Crafting planner for **Graveyard Keeper** and **Graveyard Keeper 2**: pick what
you want to make, get the raw materials to gather, the crafts to run in order,
and the full crafting tree. Lists are saved locally. A research tree view shows
what each tech costs, needs and unlocks, with hidden techs kept spoiler-free.

The app ships **no game content**. On launch it reads recipes, techs and texts
(11 languages) straight from your own install, so it stays in sync with game
updates and nothing belonging to the publisher is redistributed.

## How it works

- `src-tauri/src/unity/` — reader for Unity `.assets` files and a typetree
  decoder. Both games store their data in a `GameBalance` object plus
  `lng_*` localization objects in `resources.assets`.
- `src-tauri/schemas/` — binary layouts of those classes. The games ship
  without typetrees, so they are generated from the game DLLs by
  `tools/schema-gen` (structure only, no content). Each class records Unity's
  layout hash: when a game update changes a layout, the app detects it, keeps
  its last good cache and asks for an app update.
- `src-tauri/src/extract/` — turns each game's data into one shared model
  (`model.rs`), cached per game and re-extracted when game files change.
- `src-tauri/src/extract/icons.rs` — icons cut from the games' sprite
  atlases (gk1: resources.assets; gk2: Addressables bundles, found by
  scanning bundle metadata), written as sheet images next to the cache.
- `src-tauri/src/save/` — follows the player's save (gk2: Odin Serializer
  binary, `knowledgeSystem` lists) to show researched techs, what can be
  researched now, and which recipes are still locked.
- `src/calc.ts` — the planner: resolves a list down to raw materials,
  aggregates shared intermediates before rounding crafts, breaks crafting
  loops, honours per-item recipe choices and owned quantities.

## Development

```sh
npm install
npm run tauri dev        # the app
npm test                 # planner tests
cd src-tauri && cargo test
```

Browser-only UI work: extract data once, then `npm run dev` serves the UI
with a mocked backend (see `src/dev/mock.ts`).

```sh
cd src-tauri
cargo run --release --example extract -- gk1 "/path/to/Graveyard Keeper" ../.dev-data/gk1.json
cargo run --release --example extract -- gk2 "/path/to/Graveyard Keeper 2" ../.dev-data/gk2.json
```

After a game update that changes a layout, regenerate the schemas:

```sh
pip install -r tools/schema-gen/requirements.txt
python tools/schema-gen/gen_schema.py gk1 "/path/to/Graveyard Keeper"
python tools/schema-gen/gen_schema.py gk2 "/path/to/Graveyard Keeper 2"
```

## Roadmap

- Reading GK1 saves (custom compressed format)
- Generating schemas at runtime from the game DLLs, so layout changes need no app update
