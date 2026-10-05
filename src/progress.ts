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
  const revealedTechs = set("revealedTechs");
  // gk2 lists what is hidden right now; gk1 only what was revealed.
  const hiddenTechs = p.lists.hiddenTechs ? set("hiddenTechs") : undefined;
  const crafts = set("unlockedCrafts");
  const buildings = set("unlockedBuildings");
  const blacklist = set("blackListCrafts");
  const formulas = p.lists.unlockedAlchemyFormulas ? set("unlockedAlchemyFormulas") : undefined;
  const byId = new Map(techs.map((t) => [t.id, t]));
  return {
    techState: (t) =>
      unlockedTechs.has(t.id)
        ? "unlocked"
        : t.parents.every((id) => unlockedTechs.has(id) || !byId.has(id))
          ? "available"
          : "locked",
    // The game keeps some unlocked techs in its hidden list; they are known.
    techHidden: (t) =>
      !unlockedTechs.has(t.id) && (hiddenTechs ? hiddenTechs.has(t.id) : t.hidden && !revealedTechs.has(t.id)),
    recipeUnlocked: (r) => {
      if (blacklist.has(r.id)) return false;
      // Alchemy recipes are `alchemy:<formula>`; the save lists formulas.
      if (r.runes) return !formulas || formulas.has(r.id.replace(/^alchemy:/, ""));
      if (!r.needsUnlock) return true;
      // gk1 unlocks buildings through its craft list.
      return crafts.has(r.id) || (r.kind === "building" && buildings.has(r.id));
    },
  };
}

/** Recipe id -> techs that unlock it. */
export function unlockedBy(techs: Tech[]): Map<string, Tech[]> {
  const m = new Map<string, Tech[]>();
  for (const t of techs) for (const r of t.unlocks) m.set(r, [...(m.get(r) ?? []), t]);
  return m;
}
