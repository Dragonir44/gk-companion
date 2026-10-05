import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import { buildIndex, type Index } from "./calc";
import { defaultLang } from "./i18n";
import type { CraftList, GameId, GameStatus, ListsFile, LoadResult, Progress, SaveSlot, SaveSlots, View } from "./types";

interface GameState {
  status: "idle" | "loading" | "ready" | "error";
  result?: LoadResult;
  index?: Index;
  error?: string;
}

export interface SaveState {
  supported: boolean;
  slots: SaveSlot[];
  /** Progress of the followed slot. */
  progress?: Progress;
  error?: string;
}

export const DEFAULT_COLUMNS: [number, number] = [340, 380];

/** Follow no save. */
export const NO_SAVE = "none";

interface State {
  ready: boolean;
  game: GameId;
  lang: string;
  showHidden: boolean;
  view: View;
  /** Show techs the game keeps hidden until revealed. */
  showSpoilers: boolean;
  /** Planner column widths in px (search, list); results take the rest. */
  columns: [number, number];
  statuses: GameStatus[];
  games: Partial<Record<GameId, GameState>>;
  lists: CraftList[];
  active: Partial<Record<GameId, string>>;
  saves: Partial<Record<GameId, SaveState>>;
  /** Followed save per game: slot id, NO_SAVE, or unset for the latest. */
  saveChoice: Partial<Record<GameId, string>>;

  init(): Promise<void>;
  selectGame(game: GameId): void;
  /** `silent`: keep showing the current data while loading (refreshes). */
  loadGame(game: GameId, silent?: boolean): Promise<void>;
  setGamePath(game: GameId, path: string | null): Promise<void>;
  setLang(lang: string): void;
  setShowHidden(v: boolean): void;
  setView(v: View): void;
  setShowSpoilers(v: boolean): void;
  setColumns(c: [number, number]): void;
  refreshSaves(game: GameId): Promise<void>;
  setSaveChoice(game: GameId, choice: string | undefined): void;

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
const refreshing = new Set<GameId>();

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
        prefs: {
          lang: s.lang,
          game: s.game,
          showHidden: s.showHidden,
          view: s.view,
          showSpoilers: s.showSpoilers,
          saveChoice: s.saveChoice,
          columns: s.columns,
        },
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
    view: "planner",
    showSpoilers: false,
    columns: DEFAULT_COLUMNS,
    statuses: [],
    games: {},
    lists: [],
    active: {},
    saves: {},
    saveChoice: {},

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
        view: prefs.view ?? "planner",
        showSpoilers: prefs.showSpoilers ?? false,
        saveChoice: prefs.saveChoice ?? {},
        columns: prefs.columns ?? DEFAULT_COLUMNS,
        lists: file?.lists ?? [],
        active: file?.active ?? {},
      });
      // Background re-extraction after a game update (see load_game).
      listen<GameId>("game-data-updated", (e) => void get().loadGame(e.payload, true)).catch(() => {});
      listen<{ game: GameId; code: string }>("game-data-failed", ({ payload }) =>
        set((s) => {
          const g = s.games[payload.game];
          if (!g?.result) return {};
          const result = { ...g.result, freshness: "stale" as const, warning: payload.code };
          return { games: { ...s.games, [payload.game]: { ...g, result } } };
        }),
      ).catch(() => {});
      await get().loadGame(game);
    },

    selectGame(game) {
      set({ game });
      persist();
      const g = get().games[game];
      if (!g || g.status === "error" || g.status === "idle") void get().loadGame(game);
    },

    async loadGame(game, silent = false) {
      if (!silent) set((s) => ({ games: { ...s.games, [game]: { status: "loading" } } }));
      try {
        const result = await invoke<LoadResult>("load_game", { game });
        const index = buildIndex(result.data);
        set((s) => ({ games: { ...s.games, [game]: { status: "ready", result, index } } }));
      } catch (e) {
        if (!silent) set((s) => ({ games: { ...s.games, [game]: { status: "error", error: String(e) } } }));
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

    setView(view) {
      set({ view });
      persist();
    },

    setShowSpoilers(showSpoilers) {
      set({ showSpoilers });
      persist();
    },

    setColumns(columns) {
      set({ columns });
      persist();
    },

    async refreshSaves(game) {
      if (refreshing.has(game)) return;
      refreshing.add(game);
      try {
        const { supported, slots } = await invoke<SaveSlots>("save_slots", { game });
        const choice = get().saveChoice[game];
        // A followed slot that disappeared falls back to the latest.
        const slot = choice === NO_SAVE ? undefined : (slots.find((s) => s.id === choice) ?? slots[0]);
        const prev = get().saves[game];
        let progress = slot && prev?.progress?.slot === slot.id ? prev.progress : undefined;
        let error: string | undefined;
        if (slot && (!progress || progress.modified !== slot.modified)) {
          try {
            progress = await invoke<Progress>("read_save", { game, slot: slot.id });
          } catch (e) {
            // Usually a save being written: keep what we had, retry next poll.
            error = String(e);
          }
        }
        set((s) => ({ saves: { ...s.saves, [game]: { supported, slots, progress, error } } }));
      } catch (e) {
        console.error("refreshSaves", e);
      } finally {
        refreshing.delete(game);
      }
    },

    setSaveChoice(game, choice) {
      set((s) => ({ saveChoice: { ...s.saveChoice, [game]: choice } }));
      persist();
      void get().refreshSaves(game);
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
