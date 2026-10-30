import { describe, expect, it } from "vitest";
import { bestBody, bodyFromItems, totals } from "./morgue";
import type { Entity, GameData } from "./types";

const item = (id: string, skulls: [number, number], extra: Partial<Entity> = {}): Entity => ({ id, skulls, ...extra });

// Two slots (skin, heart), one extra, fluids and collars.
const data: GameData = {
  modelVersion: 14,
  game: "gk2",
  fingerprint: "",
  unityVersion: "",
  items: [
    item("skin_0_0", [0, 0]),
    item("skin_3_0", [3, 0]),
    item("skin_1_3", [1, 3]),
    item("heart_0_1", [0, 1]),
    item("heart_2_2", [2, 2]),
    item("heart_2_0", [2, 0]),
    item("surgeon_mistake_heart", [-1, -1]),
    item("fat", [1, 0]),
    item("embalm_red", [1, 0]),
    item("embalm_white:1", [0, 1]),
    item("flesh", [0, 1]),
    item("embalm_red_white", [1, -1]),
    item("embalm_white", [0, 2]),
    { id: "collar_bronze", collar: 5 },
    { id: "organ_improver_red" },
  ],
  objects: [],
  groups: {},
  recipes: [
    {
      id: "organ_improver_red_heart",
      kind: "craft",
      stations: ["formulation_bench"],
      inputs: [
        { item: "gr_heart", count: 1, group: true },
        { item: "organ_improver_red", count: 1 },
      ],
      outputs: [{ item: "heart_2_0", count: 1 }],
      hidden: false,
      needsUnlock: true,
      mastery: 7,
    },
  ],
  techs: [],
  branches: [],
  locales: {},
  icons: { sheets: [], sheetSizes: [], sprites: {} },
  morgue: {
    slots: ["gr_skin", "gr_heart"],
    organs: [
      ["skin_0_0", "skin_3_0", "skin_1_3"],
      ["heart_0_1", "heart_2_2", "heart_2_0", "surgeon_mistake_heart"],
    ],
    extras: ["fat", "flesh"],
    embalms: ["embalm_red", "embalm_red_white", "embalm_white", "embalm_white:1"],
    collars: ["collar_bronze"],
  },
};

const body = bodyFromItems(data, ["body_zombie", "skin_0_0", "surgeon_mistake_heart", "fat", "collar_bronze"]);

describe("morgue", () => {
  it("reads a body from its items and sums its skulls", () => {
    expect(body).toEqual({ organs: ["skin_0_0", "surgeon_mistake_heart"], extras: ["fat"], embalms: [], collar: "collar_bronze" });
    expect(totals(data, body)).toEqual({ red: 0, white: -1, redKept: 0, cap: 5 });
  });

  it("goes for white: best white organs, one of each fluid at its best", () => {
    const p = bestBody(data, { ...body, collar: undefined }, "white");
    expect(p.body.organs).toEqual(["skin_1_3", "heart_2_2"]);
    // Fat takes no white away: left alone.
    expect(p.body.extras).toEqual(["fat"]);
    // embalm_white:1 is the same fluid, lower quality: not added.
    expect(p.body.embalms).toEqual(["embalm_white"]);
    expect(p.after.white).toBe(5 + 2);
  });

  it("makes room for a fluid by taking out an extra worth less", () => {
    const full = { ...body, collar: undefined, extras: ["fat", "fat", "fat", "flesh", "flesh", "flesh"] };
    const p = bestBody(data, full, "white");
    expect(p.steps.filter((s) => s.kind !== "swap" && s.kind !== "insert")).toEqual([
      { kind: "remove", item: "fat", recipe: undefined },
      { kind: "embalm", item: "embalm_white", recipe: undefined },
    ]);
  });

  it("goes for red, prefers organs without white, stops at the collar", () => {
    const p = bestBody(data, body, "red");
    // heart_2_0 over heart_2_2: same red, keeps the white heart for burials.
    expect(p.body.organs).toEqual(["skin_3_0", "heart_2_0"]);
    expect(p.body.extras).toEqual(["fat"]);
    expect(p.after).toMatchObject({ red: 6, redKept: 5 });
    expect(p.steps.some((s) => s.kind === "embalm")).toBe(false);
  });

  it("changes nothing once the collar is full", () => {
    const full = { ...body, organs: ["skin_3_0", "heart_2_2"] };
    expect(bestBody(data, full, "red").steps).toEqual([]);
  });

  it("reaches the collar with the smallest change that is enough", () => {
    // Red 1 (fat), 4 short of 5: no single change is enough, so the
    // biggest (skin +3), then the smallest that covers the last one.
    const p = bestBody(data, { ...body, organs: ["skin_0_0", "heart_0_1"] }, "red");
    expect(p.steps.map((s) => s.item)).toEqual(["skin_3_0", "heart_2_0"]);
    // One short: the +2 heart rather than the +3 skin.
    const q = bestBody(data, { ...body, organs: ["skin_0_0", "heart_0_1"], extras: ["fat", "fat", "fat", "fat"] }, "red");
    expect(q.steps.map((s) => s.item)).toEqual(["heart_2_0"]);
  });

  it("makes a missing organ with an improver, from a spare one or the body's own", () => {
    const own = { ...body, organs: ["skin_3_0", "heart_0_1"], extras: [], collar: undefined };
    const recipe = data.recipes[0];
    // A spare heart in stock: improved, then swapped in.
    const p = bestBody(data, own, "red", { organ_improver_red: 1, heart_0_1: 2 });
    expect(p.steps).toEqual([
      { kind: "improve", slot: 1, from: "heart_0_1", item: "heart_2_0", recipe },
      { kind: "swap", slot: 1, from: "heart_0_1", item: "heart_2_0", recipe: undefined },
    ]);
    expect(p.used).toEqual({ organ_improver_red: 1, heart_0_1: 1 });
    // No spare: the body's own heart is taken out and improved.
    expect(bestBody(data, own, "red", { organ_improver_red: 1 }).used).toEqual({ organ_improver_red: 1 });
    // Recipe not unlocked: no improving.
    expect(bestBody(data, own, "red", { organ_improver_red: 1 }, () => false).steps).toEqual([]);
    // No improver: nothing to do for the heart.
    expect(bestBody(data, own, "red", {}).steps).toEqual([]);
  });

  it("only uses what is in stock", () => {
    const p = bestBody(data, { ...body, collar: undefined }, "red", { heart_2_2: 1, embalm_red_white: 2 });
    expect(p.body.organs).toEqual(["skin_0_0", "heart_2_2"]);
    expect(p.used).toEqual({ heart_2_2: 1, embalm_red_white: 1 });
    expect(p.steps).toEqual([
      { kind: "swap", slot: 1, from: "surgeon_mistake_heart", item: "heart_2_2", recipe: undefined },
      { kind: "embalm", item: "embalm_red_white", recipe: undefined },
    ]);
  });
});
