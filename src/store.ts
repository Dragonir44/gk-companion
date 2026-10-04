import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

import { buildIndex, type Index } from "./calc";
import { defaultLang } from "./i18n";
import type { CraftList, GameId, GameStatus, ListsFile, LoadResult } from "./types";

interface GameState {
  status: "idle" | "loading" | "ready" | "error";
  result?: LoadResult;
  index?: Index;
  error?: string;
}

interface State {
  ready: boolean;
  game: GameId;
  lang: string;
  showHidden: boolean;
  statuses: GameStatus[];
  games: Partial<Record<GameId, GameState>>;
  lists: CraftList[];
  active: Partial<Record<GameId, string>>;

  init(): Promise<void>;
  selectGame(game: GameId): void;
  loadGame(game: GameId): Promise<void>;
  setGamePath(game: GameId, path: string | null): Promise<void>;
  setLang(lang: string): void;
  setShowHidden(v: boolean): void;

  createList(name: string): void;
  selectList(id: string): void;
  renameList(id: string, name: string): void;
  deleteList(id: string): void;
  addEntry(recipe: string): void;
  setCount(index: number, count: number): void;
  removeEntry(index: number): void;
  setChoice(item: string, choice: string | undefined): void;
  setHave(item: string, qty: number): void;
}

const newId = () => Math.random().toString(36).slice(2, 10) + Date.now().toString(36);

let saveTimer: ReturnType<typeof setTimeout> | undefined;
let initStarted = false;

export const useStore = create<State>((set, get) => {
  /** Persists lists and prefs, debounced: edits come in bursts (typing). */
  const persist = () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const s = get();
      const file: ListsFile = {
        version: 1,
        lists: s.lists,
        active: s.active,
        prefs: { lang: s.lang, game: s.game, showHidden: s.showHidden },
      };
      invoke("save_lists", { lists: file }).catch((e) => console.error("save_lists", e));
    }, 400);
  };

  /** Applies `fn` to the active list of the current game, creating one if needed. */
  const editActive = (fn: (l: CraftList) => CraftList) => {
    const s = get();
    let lists = s.lists;
    let id = s.active[s.game];
    if (!lists.some((l) => l.id === id && l.game === s.game)) {
      const fallback = lists.find((l) => l.game === s.game);
      if (fallback) id = fallback.id;
      else {
        const created = blankList(s.game, "");
        lists = [...lists, created];
        id = created.id;
      }
    }
    set({
      lists: lists.map((l) => (l.id === id ? { ...fn(l), updatedAt: Date.now() } : l)),
      active: { ...s.active, [s.game]: id },
    });
    persist();
  };

  return {
    ready: false,
    game: "gk1",
    lang: defaultLang(),
    showHidden: false,
    statuses: [],
    games: {},
    lists: [],
    active: {},

    async init() {
      // StrictMode runs effects twice in dev.
      if (initStarted) return;
      initStarted = true;
      const [file, statuses] = await Promise.all([
        invoke<ListsFile | null>("load_lists").catch(() => null),
        invoke<GameStatus[]>("game_statuses").catch(() => []),
      ]);
      const prefs = file?.prefs ?? {};
      // Default to a game that is installed.
      const installed = statuses.find((s) => s.install || s.cached)?.game;
      const game = prefs.game ?? installed ?? "gk1";
      set({
        ready: true,
        statuses,
        game,
        lang: prefs.lang ?? get().lang,
        showHidden: prefs.showHidden ?? false,
        lists: file?.lists ?? [],
        active: file?.active ?? {},
      });
      await get().loadGame(game);
    },

    selectGame(game) {
      set({ game });
      persist();
      const g = get().games[game];
      if (!g || g.status === "error" || g.status === "idle") void get().loadGame(game);
    },

    async loadGame(game) {
      set((s) => ({ games: { ...s.games, [game]: { status: "loading" } } }));
      try {
        const result = await invoke<LoadResult>("load_game", { game });
        const index = buildIndex(result.data);
        set((s) => ({ games: { ...s.games, [game]: { status: "ready", result, index } } }));
      } catch (e) {
        set((s) => ({ games: { ...s.games, [game]: { status: "error", error: String(e) } } }));
      }
    },

    async setGamePath(game, path) {
      await invoke("set_game_path", { game, path });
      set({ statuses: await invoke<GameStatus[]>("game_statuses") });
      await get().loadGame(game);
    },

    setLang(lang) {
      set({ lang });
      persist();
    },

    setShowHidden(showHidden) {
      set({ showHidden });
      persist();
    },

    createList(name) {
      const l = blankList(get().game, name);
      set((s) => ({ lists: [...s.lists, l], active: { ...s.active, [s.game]: l.id } }));
      persist();
    },

    selectList(id) {
      set((s) => ({ active: { ...s.active, [s.game]: id } }));
      persist();
    },

    renameList(id, name) {
      set((s) => ({ lists: s.lists.map((l) => (l.id === id ? { ...l, name, updatedAt: Date.now() } : l)) }));
      persist();
    },

    deleteList(id) {
      set((s) => {
        const lists = s.lists.filter((l) => l.id !== id);
        const next = lists.find((l) => l.game === s.game)?.id;
        return { lists, active: { ...s.active, [s.game]: next } };
      });
      persist();
    },

    addEntry(recipe) {
      editActive((l) => {
        const i = l.entries.findIndex((e) => e.recipe === recipe);
        if (i >= 0) {
          const entries = [...l.entries];
          entries[i] = { ...entries[i], count: entries[i].count + 1 };
          return { ...l, entries };
        }
        return { ...l, entries: [...l.entries, { recipe, count: 1 }] };
      });
    },

    setCount(index, count) {
      editActive((l) => ({
        ...l,
        entries: l.entries.map((e, i) => (i === index ? { ...e, count: Math.max(1, Math.floor(count) || 1) } : e)),
      }));
    },

    removeEntry(index) {
      editActive((l) => ({ ...l, entries: l.entries.filter((_, i) => i !== index) }));
    },

    setChoice(item, choice) {
      editActive((l) => {
        const choices = { ...l.choices };
        if (choice === undefined) delete choices[item];
        else choices[item] = choice;
        return { ...l, choices };
      });
    },

    setHave(item, qty) {
      editActive((l) => {
        const have = { ...l.have };
        if (qty > 0) have[item] = Math.floor(qty);
        else delete have[item];
        return { ...l, have };
      });
    },
  };
});

function blankList(game: GameId, name: string): CraftList {
  return { id: newId(), name, game, entries: [], choices: {}, have: {}, updatedAt: Date.now() };
}

/** Active list of the current game, if any. */
export function useActiveList(): CraftList | undefined {
  return useStore((s) => {
    const id = s.active[s.game];
    return s.lists.find((l) => l.id === id && l.game === s.game) ?? s.lists.find((l) => l.game === s.game);
  });
}
