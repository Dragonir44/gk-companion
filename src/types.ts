// Mirrors src-tauri/src/model.rs.

export type GameId = "gk1" | "gk2";
export type View = "planner" | "research" | "sites" | "stock" | "morgue" | "zombies";
export const GAMES: GameId[] = ["gk1", "gk2"];

export interface Entity {
  id: string;
  name?: string;
  desc?: string;
  heavy?: boolean;
  icon?: string;
  /** Alchemy runes [red, green, blue] (gk2). */
  runes?: [number, number, number];
  /** Skulls [red, white] the item brings to a body (gk2). */
  skulls?: [number, number];
  /** Zombie collar: most red skulls the zombie keeps. */
  collar?: number;
  tool?: Tool;
  /** Work station: the talent its crafts use (gk2). */
  work?: Work;
}

export interface Tool {
  /** Tool type; a station takes tools of its type (`Work.tool`). */
  kind: number;
  /** Talents it adds `bonus` mastery to. */
  talents: string[];
  bonus: number;
  /** The player's starting (rusty) tools: zombies can't hold them. */
  starter?: boolean;
}

/** `Work.tool` of stations worked with any tool of their talent. */
export const ANY_TOOL = 10;

export interface Work {
  talent: string;
  tool: number;
  /** A zombie can be assigned to it. */
  zombie?: boolean;
}

/** Body parts by role (gk2); skull counts are on the items. */
export interface Morgue {
  /** Organ slots as item groups (gr_skin...). */
  slots: string[];
  /** Organs per slot, same order; surgeon mistakes included. */
  organs: string[][];
  /** Optional parts (flesh, fat, blood). */
  extras: string[];
  embalms: string[];
  collars: string[];
}

/** Node of the zombie talent tree; each one takes one red skull. */
export interface TalentNode {
  id: string;
  talent: string;
  parents: string[];
  x: number;
  y: number;
  cost: Record<string, number>;
  /** Mastery added to the tree's talent. */
  mastery?: number;
  perk?: string;
  name?: string;
  desc?: string;
  icon?: string;
  /** On crafts listing the perk: extra mastery, work ticks saved. */
  masteryBonus?: number;
  speedBonus?: number;
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
  /** Alchemy formula: runes [red, green, blue] its mix must total. */
  runes?: [number, number, number];
  /** One-off job on a world object: a construction site (gk2). */
  site?: boolean;
  /** Mastery needed in the station's talent (gk2). */
  mastery?: number;
  /** Perks that help with this craft. */
  perks?: string[];
}

/** One valid alchemy mix: one of each item. */
export interface Mix {
  id: string;
  items: string[];
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
  /** Steam build id of the extracted game version. */
  gameBuild?: string;
  unityVersion: string;
  items: Entity[];
  objects: Entity[];
  groups: Record<string, string[]>;
  recipes: Recipe[];
  techs: Tech[];
  branches: Branch[];
  zones?: Entity[];
  locales: Record<string, Record<string, string>>;
  icons: IconIndex;
  /** Alchemy recipe id -> mixes that make it. */
  alchemyMixes?: Record<string, Mix[]>;
  /** The in-game world map (gk2). */
  map?: WorldMap;
  morgue?: Morgue;
  /** The zombies' talent tree (gk2), shared by every zombie. */
  zombieTree?: TalentNode[];
}

export interface WorldMap {
  /** Sprite of the map image in the icon sheets. */
  sprite: string;
  /** Ground rectangle [x, z] the image covers. */
  worldMin: [number, number];
  worldMax: [number, number];
  /** Interior zone -> fixed point on the image, fractions from top-left. */
  zones: Record<string, [number, number]>;
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
  freshness: "extracted" | "cached" | "stale" | "refreshing";
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
  prefs: {
    lang?: string;
    game?: GameId;
    showHidden?: boolean;
    view?: View;
    showSpoilers?: boolean;
    saveChoice?: Partial<Record<GameId, string>>;
    columns?: [number, number];
    useStock?: boolean;
    onlyAvailable?: boolean;
  };
}

// --- Save progress -----------------------------------------------------------

export interface SaveSlot {
  id: string;
  path: string;
  /** ms since epoch; changes on every save. */
  modified: number;
  info?: { day?: number; isAutoSave?: boolean; saveDateTime?: string } | null;
}

export interface SaveSlots {
  supported: boolean;
  slots: SaveSlot[];
}

export interface Progress {
  slot: string;
  modified: number;
  /** knowledgeSystem lists: unlockedTechs, revealedTechs, hiddenTechs, unlockedCrafts... */
  lists: Record<string, string[]>;
  /** Stored items per container (gk2). */
  inventories?: Container[];
  /** World object instances with their ground position (gk2). */
  objects?: WorldObject[];
  zombies?: Zombie[];
}

export interface Zombie {
  /** Text key of its name. */
  name: string;
  object: string;
  zone?: string;
  /** World object id of the station it works at. */
  station?: string;
  /** Everything in its body: organs, extras, embalming, collar, tools. */
  items: string[];
  tool?: string;
  collar?: string;
  /** Talent tree nodes bought. */
  studied: string[];
  /** Points left to spend: r, g, b. */
  points: Record<string, number>;
}

export interface WorldObject {
  id: string;
  zone?: string;
  /** [x, z] */
  pos: [number, number];
}

export interface Container {
  /** World object id (`chest_rough`), or `player`. */
  object: string;
  /** World zone id; named in GameData.zones. */
  zone?: string;
  items: Record<string, number>;
}

/** Which save to follow: a slot id, "none", or undefined for the latest. */
export type SaveChoice = string | undefined;
