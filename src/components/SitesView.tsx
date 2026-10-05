import { useEffect, useMemo, useRef, useState } from "react";

import { fmt } from "../i18n";
import { useStore } from "../store";
import type { Recipe, WorldObject } from "../types";
import { type MapPoint, placeOnMap } from "../worldmap";
import { useGame } from "./ctx";
import { Icon } from "./Icon";
import { type Landmark, MapPanel, type Marker } from "./MapPanel";
import { LockBadge } from "./Search";

/** Objects with no zone are placed by their nearest zoned neighbour. */
const NEAR_DISTANCE = 40;

interface Site {
  key: string;
  object: WorldObject;
  recipes: Recipe[];
  /** Zone id, and whether it was guessed from a neighbour. */
  zone?: string;
  near: boolean;
  /** On the game's map, and the number shown on it. */
  at?: MapPoint;
  num?: number;
}

const TELEPORT = /^teleport_milestone_\d+_/;

/** Construction sites left in the save (repairs, blockages, town plots), by zone. */
export function SitesView() {
  const { t, n, idx } = useGame();
  const game = useStore((s) => s.game);
  const progress = useStore((s) => s.saves[s.game]?.progress);
  const lists = useStore((s) => s.lists).filter((l) => l.game === game);
  const activeId = useStore((s) => s.active[s.game]);
  const addRecipes = useStore((s) => s.addRecipes);
  const setView = useStore((s) => s.setView);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [target, setTarget] = useState<string>(activeId ?? "");
  const [done, setDone] = useState<string | null>(null);
  // Site under the pointer in the list, and the one picked (map or list).
  const [hovered, setHovered] = useState<string>();
  const [focused, setFocused] = useState<string>();
  const cards = useRef(new Map<string, HTMLDivElement>());
  const map = idx.data.map;

  const sites = useMemo(() => {
    const byStation = new Map<string, Recipe[]>();
    for (const r of idx.data.recipes) {
      if (!r.site || r.hidden || !(r.inputs.length || r.runes)) continue;
      for (const st of r.stations) byStation.set(st, [...(byStation.get(st) ?? []), r]);
    }
    const objects = progress?.objects ?? [];
    const zoned = objects.filter((o) => o.zone);
    const nearest = (o: WorldObject) => {
      let best: WorldObject | undefined;
      let bestD = NEAR_DISTANCE;
      for (const z of zoned) {
        const d = Math.hypot(z.pos[0] - o.pos[0], z.pos[1] - o.pos[1]);
        if (d < bestD) [best, bestD] = [z, d];
      }
      return best?.zone;
    };
    return objects
      .filter((o) => byStation.has(o.id))
      .map((o, i): Site => {
        const guess = o.zone ? undefined : nearest(o);
        const at = map ? placeOnMap(map, o) : undefined;
        return { key: `${i}`, object: o, recipes: byStation.get(o.id)!, zone: o.zone ?? guess, near: !o.zone && !!guess, at };
      });
  }, [idx, progress, map]);

  const zoneName = (z?: string) => {
    const e = z ? idx.data.zones?.find((x) => x.id === z) : undefined;
    const name = (e && n.text(e.name)) || z || t.sitesElsewhere;
    // Town districts are only named by their battle: say it's the town.
    return z?.startsWith("wz_fight_") ? `${t.town} · ${name}` : name;
  };
  const groups = useMemo(() => {
    const m = new Map<string, Site[]>();
    for (const s of sites) {
      const label = s.zone ? zoneName(s.zone) : t.sitesElsewhere;
      m.set(label, [...(m.get(label) ?? []), s]);
    }
    for (const g of m.values()) g.sort((a, b) => a.object.pos[0] - b.object.pos[0]);
    const sorted = [...m].sort((a, b) => a[0].localeCompare(b[0]));
    // Numbered in list order, so the map reads alongside the list.
    let num = 0;
    for (const [, list] of sorted) for (const s of list) s.num = s.at ? ++num : undefined;
    return sorted;
  }, [sites, t, n, idx]);

  // Sites sharing a point (an interior's) share a marker.
  const markerOf = useMemo(() => {
    const byPoint = new Map<string, Site[]>();
    for (const [, list] of groups) {
      for (const s of list) {
        if (!s.at) continue;
        const k = s.at.map((v) => v.toFixed(4)).join(",");
        byPoint.set(k, [...(byPoint.get(k) ?? []), s]);
      }
    }
    const of = new Map<string, Site[]>();
    for (const members of byPoint.values()) for (const s of members) of.set(s.key, members);
    return of;
  }, [groups]);
  const markers: Marker[] = useMemo(() => {
    const seen = new Set<Site[]>();
    const out: Marker[] = [];
    for (const members of markerOf.values()) {
      if (seen.has(members)) continue;
      seen.add(members);
      const [first] = members;
      out.push({
        key: first.key,
        at: first.at!,
        label: members.length > 1 ? `${first.num}+${members.length - 1}` : `${first.num}`,
        title: members.map((s) => `${s.num}. ${n.name(s.object.id)}`).join("\n"),
        selected: members.some((s) => [...selected].some((k) => k.startsWith(`${s.key}|`))),
      });
    }
    return out;
  }, [markerOf, selected, n]);
  const landmarks: Landmark[] = useMemo(() => {
    if (!map) return [];
    return (progress?.objects ?? []).flatMap((o) => {
      if (!TELEPORT.test(o.id)) return [];
      const at = placeOnMap(map, o);
      if (!at) return [];
      const zone = o.id.replace(TELEPORT, "");
      const e = idx.data.zones?.find((z) => z.id === zone);
      const name = (e && n.text(e.name)) || zone.replace(/_/g, " ");
      return [{ at, title: `${t.mapTeleport} · ${name}` }];
    });
  }, [map, progress, idx, n, t]);
  const markerKey = (site?: string) => (site ? markerOf.get(site)?.[0].key : undefined);

  // A site picked on the map: bring its card into view.
  useEffect(() => {
    if (focused) cards.current.get(focused)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focused]);

  if (!progress?.objects?.length) return <section className="panel sites-view muted center-text">{t.sitesNone}</section>;

  const pick = (site: Site, r: Recipe) => {
    const key = `${site.key}|${r.id}`;
    setSelected((prev) => {
      const next = new Set(prev);
      // One job per site: picking another building replaces the choice.
      for (const k of next) if (k.startsWith(`${site.key}|`) && k !== key) next.delete(k);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };
  const add = () => {
    const recipes = [...selected].map((k) => k.split("|")[1]);
    const name = t.sitesListName;
    addRecipes(target || null, recipes, name);
    setDone(fmt(t.sitesAdded, { n: recipes.length }));
    setSelected(new Set());
  };

  return (
    <section className="panel sites-view">
      <p className="muted small">{t.sitesIntro}</p>
      <div className={map ? "sites-split" : "sites-split no-map"}>
      {map && (
        <MapPanel
          markers={markers}
          landmarks={landmarks}
          focused={markerKey(focused)}
          hovered={markerKey(hovered)}
          onPick={setFocused}
        />
      )}
      <div className="sites-body">
        {groups.map(([label, list]) => (
          <div key={label} className="sites-group">
            <h3>
              {label} <span className="muted">({list.length})</span>
            </h3>
            <div className="sites-grid">
              {list.map((site) => (
                <div
                  key={site.key}
                  className={site.key === focused ? "site focus" : "site"}
                  ref={(el) => {
                    if (el) cards.current.set(site.key, el);
                    else cards.current.delete(site.key);
                  }}
                  onMouseEnter={() => setHovered(site.key)}
                  onMouseLeave={() => setHovered(undefined)}
                >
                  <div className="site-head">
                    {map &&
                      (site.num ? (
                        <button className="site-num" title={t.mapShow} onClick={() => setFocused(site.key)}>
                          {site.num}
                        </button>
                      ) : (
                        <span className="site-num off" title={t.mapNotOnMap}>
                          ?
                        </span>
                      ))}
                    <Icon sprite={n.icon(site.object.id)} size={22} />
                    <strong>{n.name(site.object.id)}</strong>
                    {site.near && <span className="muted small">({t.sitesNearHint})</span>}
                  </div>
                  {site.recipes.map((r) => {
                    const on = selected.has(`${site.key}|${r.id}`);
                    return (
                      <button key={r.id} className={on ? "site-job on" : "site-job"} onClick={() => pick(site, r)} aria-pressed={on}>
                        <span className="site-job-name">
                          <span className="check">{on ? "☑" : "☐"}</span>
                          <Icon sprite={n.recipeIcon(r)} size={18} />
                          <LockBadge r={r} />
                          {n.recipe(r)}
                        </span>
                        <span className="site-inputs">
                          {r.inputs.map((i) => (
                            <span key={i.item} className="chip" title={n.name(i.item)}>
                              <Icon sprite={n.icon(i.item)} size={14} />
                              {i.count}
                            </span>
                          ))}
                        </span>
                      </button>
                    );
                  })}
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
      </div>
      <footer className="sites-bar">
        <span>{fmt(t.sitesSelected, { n: selected.size })}</span>
        <label>
          {t.sitesAddTo}{" "}
          <select value={target} onChange={(e) => setTarget(e.target.value)}>
            {lists.map((l) => (
              <option key={l.id} value={l.id}>
                {l.name || t.defaultListName}
              </option>
            ))}
            <option value="">{t.newList}</option>
          </select>
        </label>
        <button className="primary" disabled={!selected.size} onClick={add}>
          {t.add}
        </button>
        {done && (
          <span className="muted">
            {done}{" "}
            <button className="link" onClick={() => setView("planner")}>
              {t.sitesOpenPlanner}
            </button>
          </span>
        )}
      </footer>
    </section>
  );
}
