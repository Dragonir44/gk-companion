// Display names for game ids, from the extracted localization.

import type { Entity, GameData, Recipe } from "./types";

export interface Namer {
  /** Text of a localization key, in the current language. */
  text(key?: string): string | undefined;
  /** Icon sprite of an item or object id. */
  icon(id: string): string | undefined;
  /** Icon of a recipe: its own, else what it makes or acts on. */
  recipeIcon(r: Recipe): string | undefined;
  name(id: string): string;
  desc(id: string): string | undefined;
  recipe(r: Recipe): string;
  station(r: Recipe): string | undefined;
}

/** `mf_anvil_2` -> "Mf anvil 2": last resort for ids without a text. */
function prettify(id: string): string {
  const s = id.replace(/[:_]+/g, " ").trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Quality variants end with `:1`..`:3` (gk1) — shown as stars. */
function quality(id: string): string {
  const m = /:(\d)$/.exec(id);
  return m ? " " + "★".repeat(Number(m[1])) : "";
}

const TALENT_MARKS: Record<string, string> = { red: "🔴", green: "🟢", blue: "🔵", orange: "🟠", yellow: "🟡" };

const RUNE_MARKS: Record<string, string> = { r: "🔴", g: "🟢", b: "🔵" };

/** Game texts embed TextMeshPro sprites (gk2 talent colours) and tags. */
function clean(t: string): string {
  return t
    // Some texts carry escapes literally ("Fournitures\u00A0: ...").
    .replace(/\\u([0-9a-fA-F]{4})/g, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/<sprite name="talent_(\w+)">/g, (_, c) => TALENT_MARKS[c] ?? "")
    .replace(/<sprite name="rune_([rgb])">/g, (_, c) => RUNE_MARKS[c])
    .replace(/<sprite[^>]*>|<\/?(b|i|u|color|size)(=[^>]*)?>/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** `heavySuffix` disambiguates heavy items from their same-named pieces. */
export function namer(data: GameData, lang: string, heavySuffix = ""): Namer {
  const entities = new Map<string, Entity>();
  for (const e of data.items) entities.set(e.id, e);
  for (const e of data.objects) if (!entities.has(e.id)) entities.set(e.id, e);
  for (const t of data.techs) entities.set(t.id, t);
  const table = data.locales[lang] ?? {};
  const en = data.locales.en ?? {};
  const text = (key?: string) => (key ? table[key] || en[key] : undefined);

  const name = (id: string): string => {
    const e = entities.get(id);
    const t = text(e?.name);
    // Unnamed "any of" groups (gr_wine): named after a member, sans quality.
    const members = data.groups[id];
    if (!t && members?.length) return name(members[0]).replace(/ ★+$/, "");
    const heavy = e && "heavy" in e && e.heavy && heavySuffix ? ` (${heavySuffix})` : "";
    return (t ? clean(t) + quality(id) : prettify(id)) + heavy;
  };

  const icon = (id: string): string | undefined => {
    const e = entities.get(id);
    if (e && "icon" in e && e.icon) return e.icon;
    const members = data.groups[id];
    return members?.length ? icon(members[0]) : undefined;
  };

  return {
    text: (key) => {
      const v = text(key);
      return v ? clean(v) : undefined;
    },
    icon,
    recipeIcon: (r) =>
      r.icon ?? (r.builds && icon(r.builds)) ?? (r.outputs[0] && icon(r.outputs[0].item)) ?? (r.stations[0] && icon(r.stations[0])) ?? undefined,
    name,
    desc: (id) => text(entities.get(id)?.desc),
    // Own name first; crafts acting on the world (repairs, clearing) are
    // otherwise named after the object they act on.
    recipe: (r) => {
      const own = text(r.name);
      if (own) return clean(own);
      if (r.builds) return name(r.builds);
      if (r.outputs[0]) return name(r.outputs[0].item);
      // Placing a decoration (gk1/gk2 graves): named after what is placed.
      const station = r.stations[0] && entities.get(r.stations[0])?.name ? name(r.stations[0]) : undefined;
      return station ?? (r.inputs[0] ? name(r.inputs[0].item) : prettify(r.id));
    },
    station: (r) => (r.stations[0] ? name(r.stations[0]) : undefined),
  };
}
