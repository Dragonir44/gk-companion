// Zombie workers: the talent tree nodes a zombie needs for a job.
// A craft needs `Recipe.mastery` in its station's talent; a zombie's mastery
// is its tool's bonus, plus the mastery nodes bought in that talent's tree,
// plus the bonus of bought perks the craft lists. Every node takes one of
// the zombie's red skulls and costs points it earns by working.
// Pure functions over GameData; no React, no Tauri.

import { ANY_TOOL, type Entity, type GameData, type Recipe, type TalentNode } from "./types";

export interface ZombieState {
  tool?: string;
  /** Nodes already bought. */
  studied: string[];
}

export interface Job {
  station: Entity;
  crafts: Recipe[];
}

export interface Option {
  /** Tool held; undefined for none. */
  tool?: string;
  toolBonus: number;
  /** Nodes to buy, parents first. */
  nodes: TalentNode[];
  cost: Record<string, number>;
  /** Red skulls the zombie needs: one per node, bought before or now. */
  skulls: number;
  /** Every craft of the job is reachable. */
  reached: boolean;
  /** Lowest mastery margin over the job's crafts (negative: missing). */
  margin: number;
  /** Work ticks saved per craft by bought perks, summed over the job. */
  speed: number;
}

/** Whether `tool` gives its bonus at `station`. */
export function fits(tool: Entity | undefined, station: Entity): boolean {
  const w = station.work;
  if (!tool?.tool || !w) return false;
  return tool.tool.talents.includes(w.talent) && (w.tool === ANY_TOOL || tool.tool.kind === w.tool);
}

/** Tools that work at the station, best first. */
export function toolsFor(data: GameData, station: Entity): Entity[] {
  return data.items.filter((i) => i.tool?.bonus && fits(i, station)).sort((a, b) => b.tool!.bonus - a.tool!.bonus);
}

/** Mastery for `craft` with these nodes and tool bonus. */
/** A perk counts once, even when two nodes grant it (factory operative). */
export function masteryFor(nodes: Iterable<TalentNode>, talent: string, toolBonus: number, craft: Recipe): number {
  let m = toolBonus;
  const perks = new Set<string>();
  for (const n of nodes) {
    if (n.talent === talent) m += n.mastery ?? 0;
    if (n.perk && !perks.has(n.perk) && craft.perks?.includes(n.perk)) {
      perks.add(n.perk);
      m += n.masteryBonus ?? 0;
    }
  }
  return m;
}

function speedOf(nodes: Iterable<TalentNode>, crafts: Recipe[]): number {
  const perks = new Map<string, number>();
  for (const n of nodes) if (n.perk && n.speedBonus) perks.set(n.perk, n.speedBonus);
  let s = 0;
  for (const [perk, bonus] of perks) s += bonus * crafts.filter((c) => c.perks?.includes(perk)).length;
  return s;
}

const total = (cost: Record<string, number>) => Object.values(cost).reduce((a, b) => a + b, 0);

/**
 * The cheapest nodes to buy for the job with a given tool: fewest nodes
 * (red skulls), then fewest points. With `speed`, every perk that speeds
 * up one of the job's crafts is bought too. When the job can't be reached
 * with this tool, every useful node is bought and `reached` is false.
 */
export function planFor(data: GameData, job: Job, state: ZombieState, tool: Entity | undefined, speed = false): Option {
  const tree = data.zombieTree ?? [];
  const byId = new Map(tree.map((n) => [n.id, n]));
  const talent = job.station.work?.talent ?? "";
  const toolBonus = fits(tool, job.station) ? tool!.tool!.bonus : 0;
  const owned = new Set(state.studied.filter((id) => byId.has(id)));
  const listed = (n: TalentNode) => !!n.perk && job.crafts.some((c) => c.perks?.includes(n.perk!));
  const speedNode = (n: TalentNode) => listed(n) && (n.speedBonus ?? 0) > 0;
  const useful = (n: TalentNode) => (n.talent === talent && (n.mastery ?? 0) > 0) || (listed(n) && (n.masteryBonus ?? 0) > 0) || (speed && speedNode(n));

  // Useful nodes and what they need, parents first.
  const wanted = new Set<string>();
  const add = (id: string) => {
    const n = byId.get(id);
    if (!n || wanted.has(id)) return;
    wanted.add(id);
    n.parents.forEach(add);
  };
  tree.filter(useful).forEach((n) => add(n.id));
  const order: TalentNode[] = [];
  const placed = new Set<string>();
  const place = (n: TalentNode) => {
    if (placed.has(n.id)) return;
    placed.add(n.id);
    n.parents.map((p) => byId.get(p)).forEach((p) => p && wanted.has(p.id) && place(p));
    order.push(n);
  };
  tree.filter((n) => wanted.has(n.id)).forEach(place);
  const candidates = order.filter((n) => !owned.has(n.id));

  const margin = (set: Set<string>) => {
    const nodes = [...set].map((id) => byId.get(id)!);
    return Math.min(...job.crafts.map((c) => masteryFor(nodes, talent, toolBonus, c) - (c.mastery ?? 0)), Infinity);
  };
  const speedPerks = new Set(speed ? tree.filter(speedNode).map((n) => n.perk!) : []);
  const done = (set: Set<string>) => {
    if (margin(set) < 0) return false;
    const have = new Set([...set].map((id) => byId.get(id)!.perk));
    return [...speedPerks].every((p) => have.has(p));
  };

  // Depth-first over the candidates in parent-first order: a node can only
  // be bought once its parents are. Pruned on the best node count so far.
  let best: string[] | undefined;
  let bestCost = Infinity;
  const chosen: string[] = [];
  const set = new Set(owned);
  const visit = (k: number) => {
    if (best && chosen.length > best.length) return;
    if (done(set)) {
      const cost = total(costOf(chosen));
      if (!best || chosen.length < best.length || cost < bestCost) {
        best = [...chosen];
        bestCost = cost;
      }
      return;
    }
    if (k === candidates.length) return;
    const n = candidates[k];
    if (n.parents.every((p) => set.has(p) || !byId.has(p))) {
      set.add(n.id);
      chosen.push(n.id);
      visit(k + 1);
      chosen.pop();
      set.delete(n.id);
    }
    visit(k + 1);
  };
  const costOf = (ids: string[]) => {
    const c: Record<string, number> = {};
    for (const id of ids) for (const [k, v] of Object.entries(byId.get(id)!.cost)) c[k] = (c[k] ?? 0) + v;
    return c;
  };
  // Out of reach even with every useful node: no search.
  if (done(new Set([...owned, ...candidates.map((n) => n.id)]))) visit(0);
  const reached = !!best;
  const buy = best ?? candidates.map((n) => n.id);
  const final = new Set([...owned, ...buy]);
  return {
    tool: tool?.id,
    toolBonus,
    nodes: buy.map((id) => byId.get(id)!),
    cost: costOf(buy),
    skulls: final.size,
    reached,
    margin: margin(final),
    speed: speedOf([...final].map((id) => byId.get(id)!), job.crafts),
  };
}

/** One option per tool that fits the station (and none), best first. */
export function zombiePlans(data: GameData, job: Job, state: ZombieState, tools: Entity[], speed = false): Option[] {
  const options = [undefined, ...tools].map((t) => planFor(data, job, state, t, speed));
  return options.sort(
    (a, b) =>
      Number(b.reached) - Number(a.reached) ||
      a.nodes.length - b.nodes.length ||
      total(a.cost) - total(b.cost) ||
      b.margin - a.margin,
  );
}

/** The cheapest collar that keeps `skulls` red skulls. */
export function collarFor(data: GameData, skulls: number): Entity | undefined {
  return data.items
    .filter((i) => i.collar !== undefined && i.collar >= skulls)
    .sort((a, b) => a.collar! - b.collar!)[0];
}
