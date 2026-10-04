import { describe, expect, it } from "vitest";
import { buildIndex, plan, RAW, tree } from "./calc";
import type { GameData, Recipe } from "./types";

const r = (id: string, inputs: [string, number][], outputs: [string, number][], extra: Partial<Recipe> = {}): Recipe => ({
  id,
  kind: "craft",
  stations: ["bench"],
  inputs: inputs.map(([item, count]) => ({ item, count })),
  outputs: outputs.map(([item, count]) => ({ item, count })),
  hidden: false,
  needsUnlock: false,
  ...extra,
});

const game = (recipes: Recipe[]): GameData => ({
  modelVersion: 1,
  game: "gk1",
  fingerprint: "",
  unityVersion: "",
  items: [],
  objects: [],
  groups: {},
  recipes,
  techs: [],
  locales: {},
});

// ore -> ingot (1:1), ingot -> 4 nails, plank + 2 nails -> crate
const basic = game([
  r("ingot", [["ore", 1]], [["ingot", 1]], { points: { r: 1 }, time: 2 }),
  r("nails", [["ingot", 1]], [["nails", 4]]),
  r("crate", [["plank", 1], ["nails", 2]], [["crate", 1]]),
]);

describe("plan", () => {
  it("resolves down to raw materials, rounding crafts up", () => {
    const p = plan(buildIndex(basic), [{ recipe: "crate", count: 3 }]);
    // 6 nails -> 2 nails crafts -> 2 ingots -> 2 ore; 2 nails left over.
    expect(Object.fromEntries(p.raw)).toEqual({ plank: 3, ore: 2 });
    expect(Object.fromEntries(p.surplus)).toEqual({ nails: 2 });
    expect(p.steps.map((s) => [s.recipe.id, s.crafts])).toEqual([
      ["ingot", 2],
      ["nails", 2],
      ["crate", 3],
    ]);
    expect(p.points).toEqual({ r: 2 });
    expect(p.time).toBe(4);
  });

  it("aggregates shared intermediates before rounding", () => {
    const g = game([...basic.recipes, r("box", [["nails", 2]], [["box", 1]])]);
    const p = plan(buildIndex(g), [
      { recipe: "crate", count: 1 },
      { recipe: "box", count: 1 },
    ]);
    // 4 nails total = exactly one nails craft, not two.
    expect(p.steps.find((s) => s.recipe.id === "nails")?.crafts).toBe(1);
  });

  it("honours raw choices and owned items", () => {
    const idx = buildIndex(basic);
    expect(Object.fromEntries(plan(idx, [{ recipe: "crate", count: 1 }], { nails: RAW }).raw)).toEqual({ plank: 1, nails: 2 });
    const p = plan(idx, [{ recipe: "crate", count: 1 }], {}, { nails: 5, plank: 1 });
    expect(Object.fromEntries(p.raw)).toEqual({});
    expect(Object.fromEntries(p.used)).toEqual({ nails: 2, plank: 1 });
  });

  it("breaks cycles by treating the looping item as raw", () => {
    const g = game([r("a", [["b", 1]], [["a", 1]]), r("b", [["a", 1]], [["b", 1]]), r("top", [["a", 1]], [["top", 1]])]);
    const p = plan(buildIndex(g), [{ recipe: "top", count: 1 }]);
    expect(p.cycles.size).toBe(1);
    expect(p.raw.size).toBe(1);
  });

  it("ignores recipes that consume their own output and hidden recipes", () => {
    const g = game([
      r("wash", [["nails", 1], ["water", 1]], [["nails", 1]]),
      r("secret", [["gold", 1]], [["plank", 1]], { hidden: true }),
      ...basic.recipes,
    ]);
    const idx = buildIndex(g);
    expect(idx.producers.get("nails")?.map((x) => x.id)).toEqual(["nails"]);
    expect(idx.producers.has("plank")).toBe(false);
  });

  it("keeps group ingredients apart", () => {
    const g = game([r("soup", [["gr_veg", 2]], [["soup", 1]])]);
    g.recipes[0].inputs[0].group = true;
    const p = plan(buildIndex(g), [{ recipe: "soup", count: 3 }]);
    expect(Object.fromEntries(p.groups)).toEqual({ gr_veg: 6 });
  });
});

describe("productions", () => {
  it("are never the default, only an explicit choice", () => {
    const g = game([r("mine", [], [["ore", 1]]), ...basic.recipes]);
    const idx = buildIndex(g);
    expect(Object.fromEntries(plan(idx, [{ recipe: "ingot", count: 2 }]).raw)).toEqual({ ore: 2 });
    const p = plan(idx, [{ recipe: "ingot", count: 2 }], { ore: "mine" });
    expect(p.raw.size).toBe(0);
    expect(p.steps.find((s) => s.recipe.id === "mine")?.crafts).toBe(2);
  });
});

describe("tree", () => {
  it("expands each branch with its own rounding", () => {
    const nodes = tree(buildIndex(basic), basic.recipes[2], 1);
    expect(nodes.map((n) => [n.item, n.qty, n.crafts])).toEqual([
      ["plank", 1, undefined],
      ["nails", 2, 1],
    ]);
    expect(nodes[1].children[0]).toMatchObject({ item: "ingot", qty: 1, crafts: 1 });
  });
});
