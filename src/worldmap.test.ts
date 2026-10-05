import { describe, expect, it } from "vitest";

import type { WorldMap } from "./types";
import { placeOnMap } from "./worldmap";

const map: WorldMap = {
  sprite: "ui:world_map",
  worldMin: [-100, -200],
  worldMax: [100, 200],
  zones: { home: [0.25, 0.4], workshop_conveyor: [0.6, 0.3] },
};

describe("placeOnMap", () => {
  it("interpolates outdoor positions, north up", () => {
    expect(placeOnMap(map, { pos: [0, 0] })).toEqual([0.5, 0.5]);
    expect(placeOnMap(map, { pos: [-100, 200] })).toEqual([0, 0]);
    expect(placeOnMap(map, { pos: [50, -100] })).toEqual([0.75, 0.75]);
  });

  it("puts interiors at their zone's point", () => {
    expect(placeOnMap(map, { pos: [-150, -600], zone: "home" })).toEqual([0.25, 0.4]);
    // Zone points may carry a prefix.
    expect(placeOnMap(map, { pos: [-150, -700], zone: "conveyor" })).toEqual([0.6, 0.3]);
  });

  it("leaves unknown interiors off the map", () => {
    expect(placeOnMap(map, { pos: [-150, -800], zone: "mine" })).toBeUndefined();
    expect(placeOnMap(map, { pos: [-150, -800] })).toBeUndefined();
  });
});
