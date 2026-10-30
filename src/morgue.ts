// Morgue: a body's skulls, and the organ swaps and embalming that push it
// towards red skulls (zombies) or white ones (burial).
// Pure functions over GameData; no React, no Tauri.

import type { Entity, GameData, Recipe } from "./types";

export type Goal = "red" | "white";

/**
 * Slots for a body's other parts: flesh, fat and blood (which can only be
 * taken out) share them with embalming fluids, one of each fluid. Not in
 * the game data: from player guides, confirmed in game.
 */
export const OTHER_SLOTS = 6;

/** A fluid whatever its quality (`embalm_red:2` -> `embalm_red`). */
export const fluidOf = (id: string) => id.split(":")[0];

export interface Body {
  /** Organ per slot (`Morgue.slots` order); null for an empty slot. */
  organs: (string | null)[];
  extras: string[];
  /** Fluids already applied: they stay. */
  embalms: string[];
  collar?: string;
}

export interface Totals {
  red: number;
  white: number;
  /** Red skulls a zombie keeps: capped by its collar. */
  redKept: number;
  cap?: number;
}

export type Step =
  | { kind: "insert"; slot: number; item: string; recipe?: Recipe }
  | { kind: "swap"; slot: number; from: string; item: string; recipe?: Recipe }
  | { kind: "remove"; slot?: number; item: string; recipe?: Recipe }
  /** Turn `from` (a spare organ, or the one taken out) into `item` with an organ improver. */
  | { kind: "improve"; slot: number; from: string; item: string; recipe: Recipe }
  | { kind: "embalm"; item: string; recipe?: Recipe };

export interface MorguePlan {
  before: Totals;
  after: Totals;
  body: Body;
  steps: Step[];
  /** Items taken from stock. */
  used: Record<string, number>;
}

const NONE: [number, number] = [0, 0];

export function emptyBody(data: GameData): Body {
  return { organs: (data.morgue?.slots ?? []).map(() => null), extras: [], embalms: [] };
}

/** A body from the items it holds (a zombie from the save); other items are ignored. */
export function bodyFromItems(data: GameData, items: string[]): Body {
  const m = data.morgue;
  const body = emptyBody(data);
  if (!m) return body;
  for (const id of items) {
    const slot = m.organs.findIndex((list) => list.includes(id));
    if (slot >= 0) body.organs[slot] = id;
    else if (m.extras.includes(id)) body.extras.push(id);
    else if (m.embalms.includes(id)) body.embalms.push(id);
    else if (m.collars.includes(id)) body.collar = id;
  }
  return body;
}

function itemsById(data: GameData): Map<string, Entity> {
  return new Map(data.items.map((e) => [e.id, e]));
}

export function totals(data: GameData, body: Body): Totals {
  const items = itemsById(data);
  const parts = [...body.organs, ...body.extras, ...body.embalms].filter((i): i is string => !!i);
  const [red, white] = parts.reduce(
    ([r, w], id) => {
      const [dr, dw] = items.get(id)?.skulls ?? NONE;
      return [r + dr, w + dw];
    },
    [0, 0],
  );
  const cap = body.collar ? items.get(body.collar)?.collar : undefined;
  return { red, white, redKept: cap === undefined ? red : Math.min(red, cap), cap };
}

/**
 * The best body for the goal. Only changes that add skulls of the goal's
 * colour are made: per slot, the organ with the most (ties: fewest of the
 * other colour, kept for other bodies), extras that take some away
 * removed, then fluids not yet used, in free slots or in place of an extra
 * worth less. An organ not in stock can come from an organ improver, if
 * `canMake` allows its recipe (the save's unlocked crafts). For red, a collar caps what counts:
 * changes stop once it is reached, using the fewest and smallest ones.
 * `have` limits organs and fluids to a stock; without it everything is
 * available.
 */
export function bestBody(
  data: GameData,
  body: Body,
  goal: Goal,
  have?: Record<string, number>,
  canMake: (r: Recipe) => boolean = () => true,
): MorguePlan {
  const m = data.morgue;
  const before = totals(data, body);
  if (!m) return { before, after: before, body, steps: [], used: {} };
  const items = itemsById(data);
  const recipes = new Map(data.recipes.map((r) => [r.id, r]));
  const value = (id: string | null): [number, number] => {
    const [r, w] = (id && items.get(id)?.skulls) || NONE;
    return goal === "red" ? [r, w] : [w, r];
  };
  // Positive when `a` is better than `b`.
  const cmp = (a: string | null, b: string | null) => {
    const [ga, oa] = value(a);
    const [gb, ob] = value(b);
    return ga - gb || ob - oa;
  };
  const used: Record<string, number> = {};
  const left = (id: string) => (have ? (have[id] ?? 0) - (used[id] ?? 0) : Infinity);
  const take = (id: string, n = 1) => (used[id] = (used[id] ?? 0) + n);
  const mistakes = new Set(m.organs.flat().filter((id) => id.startsWith("surgeon_mistake_")));
  const steps: Step[] = [];
  const next: Body = { ...body, organs: [...body.organs], extras: [...body.extras], embalms: [...body.embalms] };
  const gained = () => (goal === "red" ? totals(data, next).red : totals(data, next).white);
  const cap = goal === "red" ? before.cap : undefined;
  const short = () => (cap === undefined ? Infinity : cap - gained());

  next.extras = next.extras.filter((id) => {
    const keep = value(id)[0] >= 0;
    if (!keep) steps.push({ kind: "remove", item: id, recipe: recipes.get(`extract_${id}`) });
    return keep;
  });

  // Organ improvers: any organ of a slot + an improver -> that slot's best
  // organ of the improver's colour (a recipe at the formulation bench).
  const improvers = new Map<string, Recipe>();
  for (const r of data.recipes) {
    const slot = r.inputs.find((i) => i.group && m.slots.includes(i.item));
    const out = r.outputs[0]?.item;
    if (slot && out && r.inputs.length === 2 && m.organs[m.slots.indexOf(slot.item)].includes(out)) improvers.set(out, r);
  }
  // An organ to improve: the spare one worth least, else the body's own.
  const spare = (slot: number, except: string) =>
    m.organs[slot].filter((id) => id !== except && !mistakes.has(id) && left(id) >= 1).sort((a, b) => cmp(a, b))[0];
  const improvable = (slot: number, id: string, cur: string | null) => {
    const r = improvers.get(id);
    const improver = r?.inputs.find((i) => !i.group)?.item;
    if (!r || !improver || left(improver) < 1 || !canMake(r)) return undefined;
    const stocked = spare(slot, id);
    const from = stocked ?? (cur && !mistakes.has(cur) ? cur : undefined);
    return from ? { recipe: r, improver, from, stocked: !!stocked } : undefined;
  };

  // Per slot, the best change that adds skulls (an empty slot beats a mistake).
  const changes = body.organs
    .map((cur, slot) => {
      let best: string | null = cur;
      for (const id of [null, ...m.organs[slot]]) {
        if (id === cur || (id && (mistakes.has(id) || (left(id) < 1 && !improvable(slot, id, cur))))) continue;
        if (cmp(id, best) > 0) best = id;
      }
      return { slot, cur, best, gain: value(best)[0] - value(cur)[0] };
    })
    .filter((c) => c.gain > 0)
    .sort((a, b) => b.gain - a.gain);
  while (changes.length && short() > 0) {
    // Under a cap: the smallest change that is enough, else the biggest.
    const need = short();
    let k = 0;
    while (k + 1 < changes.length && changes[k + 1].gain >= need) k++;
    const { slot, cur, best } = changes.splice(k, 1)[0];
    if (best !== null && left(best) < 1) {
      // Not in stock: made with an improver (checked again: an earlier slot may have used it).
      const imp = improvable(slot, best, cur);
      if (!imp) continue;
      take(imp.improver);
      if (imp.stocked) take(imp.from);
      steps.push({ kind: "improve", slot, from: imp.from, item: best, recipe: imp.recipe });
    } else if (best !== null) take(best);
    next.organs[slot] = best;
    if (best === null) steps.push({ kind: "remove", slot, item: cur!, recipe: recipes.get(`extract_${cur}`) });
    else
      steps.push(
        cur === null
          ? { kind: "insert", slot, item: best, recipe: recipes.get(`insert_${best}`) }
          : { kind: "swap", slot, from: cur, item: best, recipe: recipes.get(`change_${best}`) },
      );
  }

  // Best quality of each fluid first; one of each fluid per body.
  const fluids = m.embalms.filter((id) => value(id)[0] > 0 && left(id) >= 1).sort((a, b) => cmp(b, a));
  for (const id of fluids) {
    if (short() <= 0) break;
    if (next.embalms.some((e) => fluidOf(e) === fluidOf(id))) continue;
    if (next.extras.length + next.embalms.length >= OTHER_SLOTS) {
      // Full: make room by taking out the extra worth least, if less than the fluid.
      const worst = next.extras.reduce<string | undefined>((w, e) => (w === undefined || cmp(e, w) < 0 ? e : w), undefined);
      if (worst === undefined || cmp(worst, id) >= 0) continue;
      next.extras.splice(next.extras.indexOf(worst), 1);
      steps.push({ kind: "remove", item: worst, recipe: recipes.get(`extract_${worst}`) });
    }
    next.embalms.push(id);
    take(id);
    steps.push({ kind: "embalm", item: id, recipe: recipes.get(`embalm_${id}`) });
  }
  return { before, after: totals(data, next), body: next, steps, used };
}
