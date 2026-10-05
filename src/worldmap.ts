// Where world objects fall on the game's map image.

import type { WorldMap, WorldObject } from "./types";

/** A point on the map image, as fractions [0..1] from its top-left corner. */
export type MapPoint = [number, number];

/** The fixed point of an interior zone (`conveyor` is drawn as `workshop_conveyor`). */
export function zonePoint(map: WorldMap, zone: string): MapPoint | undefined {
  if (map.zones[zone]) return map.zones[zone];
  const key = Object.keys(map.zones).find((k) => k.endsWith(`_${zone}`));
  return key ? map.zones[key] : undefined;
}

/**
 * Like the game: outdoor positions are interpolated between the map's world
 * bounds; interiors (stored far outside them) take their zone's point.
 */
export function placeOnMap(map: WorldMap, o: Pick<WorldObject, "pos" | "zone">): MapPoint | undefined {
  const [x, z] = o.pos;
  const u = (x - map.worldMin[0]) / (map.worldMax[0] - map.worldMin[0]);
  const v = 1 - (z - map.worldMin[1]) / (map.worldMax[1] - map.worldMin[1]);
  if (u >= 0 && u <= 1 && v >= 0 && v <= 1) return [u, v];
  return o.zone ? zonePoint(map, o.zone) : undefined;
}
