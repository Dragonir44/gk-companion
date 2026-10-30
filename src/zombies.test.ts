import { describe, expect, it } from "vitest";
import { collarFor, fits, masteryFor, planFor, toolsFor, zombiePlans } from "./zombies";
import type { Entity, GameData, Recipe, TalentNode } from "./types";

const node = (id: string, parents: string[], extra: Partial<TalentNode>): TalentNode => ({
  id,
  talent: "talent_red",
  parents,
  x: 0,
  y: 0,
  cost: { r: 50 },
  ...extra,
});

// Red tree: +1, then +1 or a perk (+3 on linked crafts), then a speed perk.
const tree = [
  node("lvl1", [], { mastery: 1 }),
  node("lvl2", ["lvl1"], { mastery: 1, cost: { r: 100 } }),
  node("smith", ["lvl1"], { perk: "zperk_smith", masteryBonus: 3, cost: { r: 150 } }),
  node("fast", ["lvl2"], { perk: "zperk_fast", speedBonus: 2 }),
  node("smith_too", ["lvl1"], { perk: "zperk_smith", masteryBonus: 3, cost: { r: 250 } }),
];
const anvil: Entity = { id: "anvil", work: { talent: "talent_red", tool: 4, zombie: true } };
const hammer = (id: string, bonus: number): Entity => ({ id, tool: { kind: 4, talents: ["talent_red"], bonus } });
const axe: Entity = { id: "axe", tool: { kind: 1, talents: ["talent_orange"], bonus: 6 } };
const craft = (id: string, mastery: number, perks: string[] = []): Recipe => ({
  id,
  kind: "craft",
  stations: ["anvil"],
  inputs: [],
  outputs: [],
  hidden: false,
  needsUnlock: false,
  mastery,
  perks,
});
const data = {
  items: [hammer("hammer_1", 2), hammer("hammer_3", 6), axe, { id: "collar_bronze", collar: 5 }, { id: "collar_gold", collar: 10 }],
  zombieTree: tree,
} as unknown as GameData;

describe("zombies", () => {
  it("tools fit stations by type and talent", () => {
    expect(fits(hammer("h", 1), anvil)).toBe(true);
    expect(fits(axe, anvil)).toBe(false);
  });

  it("leaves out the rusty starting tools", () => {
    const rusty: Entity = { id: "hammer_0", tool: { kind: 4, talents: ["talent_red"], bonus: 1, starter: true } };
    const withRusty = { ...data, items: [...data.items, rusty] };
    expect(toolsFor(withRusty, anvil).map((t) => t.id)).toEqual(["hammer_3", "hammer_1"]);
  });

  it("counts a perk once even when two nodes grant it", () => {
    expect(masteryFor([tree[2], tree[4]], "talent_red", 0, craft("c", 1, ["zperk_smith"]))).toBe(3);
  });

  it("buys the fewest nodes, then the cheapest", () => {
    // Needs 5: hammer_1 (2) + lvl1 + smith perk (3) = 6, two nodes.
    const p = planFor(data, { station: anvil, crafts: [craft("blade", 5, ["zperk_smith"])] }, { studied: [] }, hammer("hammer_1", 2));
    expect(p.nodes.map((n) => n.id)).toEqual(["lvl1", "smith"]);
    expect(p).toMatchObject({ reached: true, skulls: 2, cost: { r: 200 }, margin: 1 });
  });

  it("keeps what the zombie has, and adds speed perks on request", () => {
    const job = { station: anvil, crafts: [craft("nails", 1, ["zperk_fast"])] };
    const p = planFor(data, job, { studied: ["lvl1"] }, undefined, true);
    expect(p.nodes.map((n) => n.id)).toEqual(["lvl2", "fast"]);
    expect(p).toMatchObject({ skulls: 3, speed: 2 });
  });

  it("says when a job is out of reach with a tool, and ranks tools", () => {
    const job = { station: anvil, crafts: [craft("crown", 8)] };
    expect(planFor(data, job, { studied: [] }, undefined).reached).toBe(false);
    const best = zombiePlans(data, job, { studied: [] }, [hammer("hammer_3", 6), hammer("hammer_1", 2)])[0];
    expect(best).toMatchObject({ tool: "hammer_3", reached: true, skulls: 2 });
  });

  it("finds the collar for a skull count", () => {
    expect(collarFor(data, 4)?.id).toBe("collar_bronze");
    expect(collarFor(data, 7)?.id).toBe("collar_gold");
    expect(collarFor(data, 12)).toBeUndefined();
  });
});
