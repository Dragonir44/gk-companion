// Browser-only dev mode: `npm run dev` outside Tauri fakes the backend with
// data previously extracted into .dev-data/<game>.json:
//   cargo run --example extract -- gk1 "<game dir>" ../.dev-data/gk1.json
// Never bundled in production builds (imported behind import.meta.env.DEV).

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

let lists: unknown = null;

export function installMocks() {
  mockWindows("main");
  mockIPC(async (cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "game_statuses":
        return ["gk1", "gk2"].map((game) => ({ game, install: { root: `.dev-data/${game}.json` }, manual: false, cached: true }));
      case "load_game": {
        const res = await fetch(`/.dev-data/${a.game}.json`);
        if (!res.ok) throw "game-not-found";
        return { data: await res.json(), freshness: "cached" };
      }
      case "load_lists":
        return lists;
      case "save_lists":
        lists = a.lists;
        return null;
      case "set_game_path":
        return null;
      default:
        throw `unmocked command ${cmd}`;
    }
  });
}
