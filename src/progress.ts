// The player's progress from a save, as lookups over game data.

import type { Progress, Recipe, Tech } from "./types";

export type TechState = "unlocked" | "available" | "locked";

export interface Unlocks {
  techState(t: Tech): TechState;
  /** Hidden in game right now (a spoiler). */
  techHidden(t: Tech): boolean;
  recipeUnlocked(r: Recipe): boolean;
}

/** Without a save: nothing known, game defaults for hidden techs. */
export const NO_PROGRESS: Unlocks = {
  techState: () => "locked",
  techHidden: (t) => t.hidden,
  recipeUnlocked: () => true,
};

export function unlocks(p: Progress | undefined, techs: Tech[]): Unlocks {
  if (!p) return NO_PROGRESS;
  const set = (k: string) => new Set(p.lists[k] ?? []);
  const unlockedTechs = set("unlockedTechs");
  const hiddenTechs = set("hiddenTechs");
  const crafts = set("unlockedCrafts");
  const buildings = set("unlockedBuildings");
  const blacklist = set("blackListCrafts");
  const byId = new Map(techs.map((t) => [t.id, t]));
  return {
    techState: (t) =>
      unlockedTechs.has(t.id)
        ? "unlocked"
        : t.parents.every((id) => unlockedTechs.has(id) || !byId.has(id))
          ? "available"
          : "locked",
    // The game keeps some unlocked techs in its hidden list; they are known.
    techHidden: (t) => hiddenTechs.has(t.id) && !unlockedTechs.has(t.id),
    recipeUnlocked: (r) => {
      if (blacklist.has(r.id)) return false;
      if (!r.needsUnlock) return true;
      return r.kind === "building" ? buildings.has(r.id) : crafts.has(r.id);
    },
  };
}

/** Recipe id -> techs that unlock it. */
export function unlockedBy(techs: Tech[]): Map<string, Tech[]> {
  const m = new Map<string, Tech[]>();
  for (const t of techs) for (const r of t.unlocks) m.set(r, [...(m.get(r) ?? []), t]);
  return m;
}
