// Mirrors src-tauri/src/model.rs.

export type GameId = "gk1" | "gk2";
export type View = "planner" | "research";
export const GAMES: GameId[] = ["gk1", "gk2"];

export interface Entity {
  id: string;
  name?: string;
  desc?: string;
  heavy?: boolean;
  icon?: string;
}

export interface Stack {
  item: string;
  count: number;
  group?: boolean;
  chance?: number;
  expr?: string;
}

export interface Recipe {
  id: string;
  name?: string;
  icon?: string;
  kind: "craft" | "building";
  stations: string[];
  inputs: Stack[];
  outputs: Stack[];
  points?: Record<string, number>;
  time?: number;
  energy?: number;
  builds?: string;
  hidden: boolean;
  needsUnlock: boolean;
}

export interface Tech {
  id: string;
  name?: string;
  desc?: string;
  branch: number;
  parents: string[];
  x: number;
  y: number;
  cost: Record<string, number>;
  unlocks: string[];
  hidden: boolean;
  icon?: string;
  /** Reputation gate rather than a research (gk2). */
  lock?: { npc: string; name?: string; value: number; portrait?: string };
}

export interface Branch {
  id: number;
  name?: string;
}

export interface GameData {
  modelVersion: number;
  game: GameId;
  fingerprint: string;
  unityVersion: string;
  items: Entity[];
  objects: Entity[];
  groups: Record<string, string[]>;
  recipes: Recipe[];
  techs: Tech[];
  branches: Branch[];
  locales: Record<string, Record<string, string>>;
  icons: IconIndex;
}

export interface IconIndex {
  sheets: string[];
  sheetSizes: [number, number][];
  /** Sprite -> [sheet, x, y, width, height], top-left origin. */
  sprites: Record<string, [number, number, number, number, number]>;
}

export interface Install {
  root: string;
  library?: string;
}

export interface GameStatus {
  game: GameId;
  install?: Install;
  manual: boolean;
  cached: boolean;
}

export interface LoadResult {
  data: GameData;
  freshness: "extracted" | "cached" | "stale";
  warning?: string;
  iconDir: string;
}

// --- Craft lists (persisted by the backend as opaque JSON) -----------------

export interface ListEntry {
  recipe: string;
  /** Number of times the recipe is made. */
  count: number;
}

export interface CraftList {
  id: string;
  name: string;
  game: GameId;
  entries: ListEntry[];
  /** Item id -> recipe id used to make it, or "raw" to gather/buy it. */
  choices: Record<string, string>;
  /** Item id -> quantity already owned. */
  have: Record<string, number>;
  updatedAt: number;
}

export interface ListsFile {
  version: 1;
  lists: CraftList[];
  active: Partial<Record<GameId, string>>;
  prefs: { lang?: string; game?: GameId; showHidden?: boolean; view?: View; showSpoilers?: boolean };
}
