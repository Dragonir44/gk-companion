import { describe, expect, it } from "vitest";
import { bestGear, fightPower, fighterPower, squadsOf, upgrade } from "./army";
import type { Entity, GameData, Zombie } from "./types";

const weapon = (id: string, power: number, damage: number, pause: number): Entity => ({ id, gear: { power, damage, pause } });
const armor = (id: string, power: number): Entity => ({ id, gear: { armor: true, power } });
const data = {
  items: [weapon("bow_2", 2, 15, 1), weapon("bow_3", 3, 22, 1), weapon("pike_3", 3, 10, 0.65), armor("armor_3", 3), armor("armor_4", 4)],
  army: {
    fights: [],
    defences: [
      { object: "tower_2_pre", power: 4, group: "towers" },
      { object: "barricade_2_pre", power: 4, group: "barricades" },
    ],
  },
} as unknown as GameData;
const items = new Map(data.items.map((e) => [e.id, e]));
const z = (place: string, weapon?: string, armor?: string): Zombie =>
  ({ name: "z", object: "zombie", station: "fighter_container", place, items: [], studied: [], points: {}, weapon, armor }) as Zombie;

describe("army", () => {
  it("a fighter needs both weapon and armour", () => {
    expect(fighterPower(items, { weapon: "bow_2", armor: "armor_3" })).toBe(5);
    expect(fighterPower(items, { weapon: "bow_2" })).toBe(0);
  });

  it("groups squads by barracks slot", () => {
    const squads = squadsOf(data, [z("a", "bow_2", "armor_3"), z("a", "bow_2", "armor_3"), z("b", "bow_3")]);
    expect(squads.map((s) => [s.place, s.fighters.length, s.power])).toEqual([
      ["a", 2, 10],
      ["b", 1, 0],
    ]);
    // Empty slots come last.
    const all = squadsOf(data, [z("a", "bow_2", "armor_3")], [{ place: "e", size: 4 }, { place: "a", size: 4 }]);
    expect(all.map((s) => [s.place, s.size, s.fighters.length])).toEqual([
      ["a", 4, 1],
      ["e", 4, 0],
    ]);
  });

  it("counts defences the battle allows and its best squads", () => {
    const fight = { id: "f", power: 20, squads: 1 };
    expect(fightPower(data, ["tower_2_pre", "barricade_2_pre"], [10, 15], fight)).toBe(8 + 15);
    expect(fightPower(data, ["tower_2_pre", "barricade_2_pre"], [10, 15], { ...fight, noTowers: true, squads: 2 })).toBe(4 + 25);
  });

  it("picks the best gear it can get, and what to make", () => {
    const best = bestGear(data, (id) => id !== "armor_4");
    // bow_3 over pike_3: same power, more damage per second.
    expect([best.weapon?.id, best.armor?.id]).toEqual(["bow_3", "armor_3"]);
    const squads = squadsOf(data, [z("a", "bow_2", "armor_3"), z("a")]);
    const up = upgrade(data, squads, best, { bow_3: 1 });
    expect(up.needs).toEqual([
      { item: "bow_3", count: 2, make: 1 },
      { item: "armor_3", count: 1, make: 1 },
    ]);
    expect([...up.fighters.values()].map((f) => f.power)).toEqual([6, 6]);
  });

  it("gears new fighters for free places when asked", () => {
    const squads = squadsOf(data, [z("a", "bow_3", "armor_3")], [{ place: "a", size: 3 }]);
    const best = bestGear(data);
    const up = upgrade(data, squads, best, {}, true);
    // armor_3 -> armor_4 for the fighter, and two new fighters fully geared.
    expect(up.needs).toEqual([
      { item: "armor_4", count: 3, make: 3 },
      { item: "bow_3", count: 2, make: 2 },
    ]);
    expect([up.free, up.powers[0], up.full[0]]).toEqual([2, 7, 21]);
  });
});
