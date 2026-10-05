import { describe, expect, it } from "vitest";
import { buildIndex, isProduction, mixKey, plan, RAW, tree } from "./calc";
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
  branches: [],
  locales: {},
  icons: { sheets: [], sheetSizes: [], sprites: {} },
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
    const p = plan(idx, [{ recipe: "crate", count: 1 }], {}, { have: { nails: 5, plank: 1 } });
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

describe("available recipes", () => {
  it("are preferred by default when the save says what is unlocked", () => {
    const g = game([...basic.recipes, r("nails_cheap", [["ingot", 1]], [["nails", 8]])]);
    const idx = buildIndex(g);
    // nails_cheap is the cheapest, but locked: the unlocked one is used.
    const locked = new Set(["nails_cheap"]);
    const p = plan(idx, [{ recipe: "crate", count: 1 }], {}, { available: (x) => !locked.has(x.id) });
    expect(p.steps.map((s) => s.recipe.id)).toContain("nails");
    expect(plan(idx, [{ recipe: "crate", count: 1 }]).steps.map((s) => s.recipe.id)).toContain("nails_cheap");
  });
});

describe("alchemy", () => {
  const lab = (extra: Partial<GameData> = {}) => {
    const g = game([
      { ...r("alchemy:elixir", [], [["elixir", 1]]), runes: [1, 5, 0] },
      r("flour", [["wheat", 2]], [["flour", 1]]),
      r("fert", [["elixir", 1], ["peat", 8]], [["fert", 1]]),
    ]);
    g.alchemyMixes = {
      "alchemy:elixir": [
        { id: "mix:flour:fragrance:clay", items: ["clay", "flour", "fragrance"] },
        { id: "mix:stick:coal:flax", items: ["stick", "coal", "flax"] },
      ],
    };
    return { ...g, ...extra };
  };

  it("makes a formula from a mix, preferring raw ingredients", () => {
    const p = plan(buildIndex(lab()), [{ recipe: "fert", count: 1 }]);
    // flour is crafted: the all-raw mix wins.
    expect(Object.fromEntries(p.raw)).toEqual({ peat: 8, stick: 1, coal: 1, flax: 1 });
    expect(p.steps.find((s) => s.item === "elixir")?.recipe.id).toBe("alchemy:elixir");
  });

  it("prefers mixes in stock, then mixes known in game, and honours a chosen mix", () => {
    const idx = buildIndex(lab());
    const inStock = plan(idx, [{ recipe: "fert", count: 1 }], {}, { have: { clay: 1, flour: 1, fragrance: 1 } });
    expect(inStock.raw.has("stick")).toBe(false);
    const known = plan(idx, [{ recipe: "fert", count: 1 }], {}, { knownMixes: new Set(["mix:flour:fragrance:clay"]) });
    expect(Object.fromEntries(known.raw)).toMatchObject({ clay: 1, fragrance: 1, wheat: 2 });
    const chosen = plan(idx, [{ recipe: "fert", count: 1 }], { [mixKey("alchemy:elixir")]: "mix:flour:fragrance:clay" });
    expect(chosen.raw.has("fragrance")).toBe(true);
  });

  it("is never taken for a zero-input production", () => {
    const idx = buildIndex(lab());
    expect(isProduction(idx.recipes.get("alchemy:elixir")!)).toBe(false);
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
