// Crafting planner: resolves a list of recipes down to raw materials.
// Pure functions over GameData; no React, no Tauri.

import type { GameData, ListEntry, Recipe, Stack } from "./types";

export const RAW = "raw";

export interface Index {
  data: GameData;
  recipes: Map<string, Recipe>;
  /** Item id -> recipes that make it, best default first. */
  producers: Map<string, Recipe[]>;
}

/** Output count of `item` per craft of `r` (guaranteed outputs only). */
export function yieldOf(r: Recipe, item: string): number {
  return r.outputs.filter((o) => o.item === item && o.chance === undefined).reduce((n, o) => n + o.count, 0);
}

export function buildIndex(data: GameData): Index {
  const recipes = new Map(data.recipes.map((r) => [r.id, r]));
  const producers = new Map<string, Recipe[]>();
  for (const r of data.recipes) {
    if (r.kind !== "craft" || r.hidden) continue;
    for (const o of r.outputs) {
      // A recipe consuming its own output (washing, repairing) can't make it.
      if (o.chance !== undefined || o.count <= 0 || r.inputs.some((i) => i.item === o.item)) continue;
      const list = producers.get(o.item) ?? [];
      if (!list.includes(r)) list.push(r);
      producers.set(o.item, list);
    }
  }
  // Default: real crafts before zero-input productions (zombie mines...),
  // then recipes whose main output is the item, then cheapest per unit.
  const cost = (r: Recipe, item: string) => r.inputs.reduce((n, i) => n + i.count, 0) / yieldOf(r, item);
  for (const [item, list] of producers) {
    list.sort(
      (a, b) =>
        Number(isProduction(a)) - Number(isProduction(b)) ||
        Number(a.outputs[0]?.item !== item) - Number(b.outputs[0]?.item !== item) ||
        cost(a, item) - cost(b, item) ||
        a.id.localeCompare(b.id),
    );
  }
  return { data, recipes, producers };
}

/** Makes items from nothing: automated gathering, not a craft to plan. */
export const isProduction = (r: Recipe) => r.inputs.length === 0;

/**
 * Recipe used to make `item`, or undefined when it is gathered raw.
 * Productions are only used when chosen explicitly.
 */
export function chosenRecipe(idx: Index, item: string, choices: Record<string, string>): Recipe | undefined {
  const c = choices[item];
  if (c === RAW) return undefined;
  if (c) {
    const r = idx.recipes.get(c);
    if (r && yieldOf(r, item) > 0) return r;
  }
  return idx.producers.get(item)?.find((r) => !isProduction(r));
}

export interface Step {
  recipe: Recipe;
  crafts: number;
  /** Item this step was planned for; undefined for list entries. */
  item?: string;
}

export interface Plan {
  /** Raw materials to gather, after what is owned. */
  raw: Map<string, number>;
  /** Ingredients given as "any item of a group". */
  groups: Map<string, number>;
  /** Owned items consumed by the plan. */
  used: Map<string, number>;
  /** Crafts in order: each after the crafts it depends on. */
  steps: Step[];
  /** Extra items produced (rounding, secondary outputs). */
  surplus: Map<string, number>;
  points: Record<string, number>;
  time: number;
  /** Items left raw because making them would loop back on themselves. */
  cycles: Set<string>;
}

const add = (m: Map<string, number>, k: string, v: number) => m.set(k, (m.get(k) ?? 0) + v);

/** Crafts needed for `need` items when one craft yields `per`. */
const craftsFor = (need: number, per: number) => Math.ceil(need / per - 1e-9);

export function plan(
  idx: Index,
  entries: ListEntry[],
  choices: Record<string, string> = {},
  have: Record<string, number> = {},
): Plan {
  const p: Plan = {
    raw: new Map(),
    groups: new Map(),
    used: new Map(),
    steps: [],
    surplus: new Map(),
    points: {},
    time: 0,
    cycles: new Set(),
  };

  // 1. Order items so every consumer comes before what it consumes
  //    (reverse DFS post-order). An item met again while still on the DFS
  //    stack closes a cycle: it is then treated as raw.
  const order: string[] = [];
  const state = new Map<string, "open" | "done">();
  const recipeOf = (item: string) => (p.cycles.has(item) ? undefined : chosenRecipe(idx, item, choices));
  const visit = (s: Stack) => {
    if (s.group) return;
    const st = state.get(s.item);
    if (st === "done") return;
    if (st === "open") {
      p.cycles.add(s.item);
      return;
    }
    state.set(s.item, "open");
    recipeOf(s.item)?.inputs.forEach(visit);
    state.set(s.item, "done");
    order.push(s.item);
  };
  const roots = entries.map((e) => ({ e, r: idx.recipes.get(e.recipe) })).filter((x) => x.r && x.e.count > 0);
  roots.forEach(({ r }) => r!.inputs.forEach(visit));
  order.reverse();

  // 2. Push demand down in that order, rounding crafts up per item.
  const demand = new Map<string, number>();
  const record = (r: Recipe, crafts: number, item?: string): Step => {
    for (const [k, v] of Object.entries(r.points ?? {})) p.points[k] = (p.points[k] ?? 0) + v * crafts;
    p.time += (r.time ?? 0) * crafts;
    for (const i of r.inputs) add(i.group ? p.groups : demand, i.item, i.count * crafts);
    // List entries keep all their outputs: those are what was asked for.
    if (item !== undefined) {
      for (const o of r.outputs) if (o.item !== item && o.chance === undefined) add(p.surplus, o.item, o.count * crafts);
    }
    return { recipe: r, crafts, item };
  };
  const entrySteps = roots.map(({ e, r }) => record(r!, e.count));
  for (const item of order) {
    let need = demand.get(item) ?? 0;
    const owned = Math.min(need, have[item] ?? 0);
    if (owned > 0) add(p.used, item, owned);
    need -= owned;
    if (need <= 0) continue;
    const r = recipeOf(item);
    if (!r) {
      add(p.raw, item, need);
      continue;
    }
    const per = yieldOf(r, item);
    const crafts = craftsFor(need, per);
    p.steps.push(record(r, crafts, item));
    const extra = crafts * per - need;
    if (extra > 0) add(p.surplus, item, extra);
  }

  // Steps were recorded consumers-first; craft order is the reverse.
  p.steps.reverse();
  p.steps.push(...entrySteps);
  return p;
}

// --- Tree view -------------------------------------------------------------

export interface TreeNode {
  item: string;
  qty: number;
  group?: boolean;
  /** Absent for raw materials. */
  recipe?: Recipe;
  crafts?: number;
  children: TreeNode[];
  /** Item would loop back onto an ancestor. */
  cycle?: boolean;
}

const MAX_DEPTH = 16;

/** Unaggregated tree of a recipe: each branch rounds its own crafts. */
export function tree(idx: Index, recipe: Recipe, crafts: number, choices: Record<string, string> = {}): TreeNode[] {
  const expand = (s: Stack, qty: number, path: Set<string>): TreeNode => {
    if (s.group) return { item: s.item, qty, group: true, children: [] };
    if (path.has(s.item) || path.size > MAX_DEPTH) return { item: s.item, qty, cycle: true, children: [] };
    const r = chosenRecipe(idx, s.item, choices);
    if (!r) return { item: s.item, qty, children: [] };
    const c = craftsFor(qty, yieldOf(r, s.item));
    const next = new Set(path).add(s.item);
    return { item: s.item, qty, recipe: r, crafts: c, children: r.inputs.map((i) => expand(i, i.count * c, next)) };
  };
  return recipe.inputs.map((i) => expand(i, i.count * crafts, new Set()));
}
