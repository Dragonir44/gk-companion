// The army: squads of zombie fighters, their gear, and the power battles
// ask for. Rules from the game's code: a fighter's power is its weapon's
// quality plus its armour's, 0 unless it has both; a squad's is the sum of
// its fighters'; a battle counts the military base's defences (unless it
// rules towers or barricades out) plus the squads sent, up to its limit.
// Pure functions over GameData; no React, no Tauri.

import type { Entity, Fight, GameData, Zombie } from "./types";

/** The barracks slots: each instance is a squad. */
export const BARRACKS = "fighter_container";

export interface Squad {
  place: string;
  /** Fighters it takes. */
  size: number;
  fighters: Zombie[];
  power: number;
}

export function fighterPower(items: Map<string, Entity>, z: Pick<Zombie, "weapon" | "armor">): number {
  const w = z.weapon ? items.get(z.weapon)?.gear : undefined;
  const a = z.armor ? items.get(z.armor)?.gear : undefined;
  return w && !w.armor && a?.armor ? w.power + a.power : 0;
}

/** Every squad slot, empty ones included (after the others). */
export function squadsOf(data: GameData, zombies: Zombie[], slots: { place: string; size: number }[] = []): Squad[] {
  const items = new Map(data.items.map((e) => [e.id, e]));
  const by = new Map<string, Zombie[]>(slots.map((s) => [s.place, []]));
  for (const z of zombies) if (z.station === BARRACKS && z.place) by.set(z.place, [...(by.get(z.place) ?? []), z]);
  const size = new Map(slots.map((s) => [s.place, s.size]));
  return [...by]
    .map(([place, fighters]) => ({
      place,
      size: Math.max(size.get(place) ?? 0, fighters.length),
      fighters,
      power: fighters.reduce((n, z) => n + fighterPower(items, z), 0),
    }))
    .sort((a, b) => Number(b.fighters.length > 0) - Number(a.fighters.length > 0));
}

/** Defences that count for a battle. */
export function defencePower(data: GameData, built: string[], fight: Fight): number {
  const defs = new Map((data.army?.defences ?? []).map((d) => [d.object, d]));
  return built.reduce((n, id) => {
    const d = defs.get(id);
    if (!d || (fight.noTowers && d.group === "towers") || (fight.noBarricades && d.group === "barricades")) return n;
    return n + d.power;
  }, 0);
}

/** Power for a battle: defences plus its best squads, up to the limit. */
export function fightPower(data: GameData, built: string[], squadPowers: number[], fight: Fight): number {
  const best = [...squadPowers].sort((a, b) => b - a).slice(0, fight.squads);
  return defencePower(data, built, fight) + best.reduce((a, b) => a + b, 0);
}

/** Damage per second, for weapons of equal power. */
const dps = (e: Entity) => (e.gear?.damage ?? 0) / (e.gear?.pause || 1);

/**
 * The best weapon and armour among what `canGet` allows (all without it):
 * most power, then (weapons) most damage per second.
 */
export function bestGear(data: GameData, canGet: (item: string) => boolean = () => true): { weapon?: Entity; armor?: Entity } {
  const gear = data.items.filter((e) => e.gear && canGet(e.id));
  const best = (list: Entity[]) => list.sort((a, b) => b.gear!.power - a.gear!.power || dps(b) - dps(a))[0];
  return { weapon: best(gear.filter((e) => !e.gear!.armor)), armor: best(gear.filter((e) => e.gear!.armor)) };
}

export interface Upgrade {
  /** Per fighter: the weapon and armour to give it (undefined: keep). */
  fighters: Map<Zombie, { weapon?: string; armor?: string; power: number }>;
  /** Items to hand out, and how many of them to make beyond the stock. */
  needs: { item: string; count: number; make: number }[];
  /** Per squad, its power once upgraded, and once also full. */
  powers: number[];
  full: number[];
  /** Free places in the squads. */
  free: number;
}

/**
 * Gives every fighter the best gear when better than what it wears; with
 * `fill`, also gears new fighters for the free places.
 */
export function upgrade(
  data: GameData,
  squads: Squad[],
  best: { weapon?: Entity; armor?: Entity },
  stock: Record<string, number> = {},
  fill = false,
): Upgrade {
  const items = new Map(data.items.map((e) => [e.id, e]));
  const fighters: Upgrade["fighters"] = new Map();
  const counts = new Map<string, number>();
  const better = (cur: string | undefined, b: Entity | undefined) =>
    b && (!cur || (items.get(cur)?.gear?.power ?? -1) < b.gear!.power) ? b.id : undefined;
  for (const z of squads.flatMap((s) => s.fighters)) {
    const weapon = better(z.weapon, best.weapon);
    const armor = better(z.armor, best.armor);
    for (const id of [weapon, armor]) if (id) counts.set(id, (counts.get(id) ?? 0) + 1);
    fighters.set(z, { weapon, armor, power: fighterPower(items, { weapon: weapon ?? z.weapon, armor: armor ?? z.armor }) });
  }
  const free = squads.reduce((n, s) => n + Math.max(0, s.size - s.fighters.length), 0);
  if (fill && free)
    for (const b of [best.weapon, best.armor]) if (b) counts.set(b.id, (counts.get(b.id) ?? 0) + free);
  const needs = [...counts].map(([item, count]) => ({ item, count, make: Math.max(0, count - (stock[item] ?? 0)) }));
  const perNew = best.weapon && best.armor ? best.weapon.gear!.power + best.armor.gear!.power : 0;
  const powers = squads.map((s) => s.fighters.reduce((n, z) => n + (fighters.get(z)?.power ?? 0), 0));
  const full = squads.map((s, k) => powers[k] + Math.max(0, s.size - s.fighters.length) * perNew);
  return { fighters, needs, powers, full, free };
}

/** Won (by the save's finished quests); undefined when the game doesn't tell. */
export function isWon(fight: Fight, finished: Set<string>): boolean | undefined {
  if (!fight.wonBy?.length) return undefined;
  return fight.wonBy.some((q) => finished.has(q));
}

/** How far a battle is: ready now, once geared, once squads are full, or out of reach. */
export type Readiness = "ready" | "gear" | "full" | "short";

export function readiness(need: number, now: number, geared: number, full: number): Readiness {
  return now >= need ? "ready" : geared >= need ? "gear" : full >= need ? "full" : "short";
}
