import { useDeferredValue, useMemo, useState } from "react";

import { fmt } from "../i18n";
import { useStore } from "../store";
import type { Recipe } from "../types";
import { Points, useGame } from "./ctx";
import { Icon } from "./Icon";

const MAX_RESULTS = 120;

/** Strips accents and case so "planche" finds "Planche" and "cle" finds "Clé". */
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function Search() {
  const { t, n, idx, u, hasSave } = useGame();
  const onlyUnlocked = useStore((s) => s.onlyAvailable);
  const setOnlyUnlocked = useStore((s) => s.setOnlyAvailable);
  const showHidden = useStore((s) => s.showHidden);
  const setShowHidden = useStore((s) => s.setShowHidden);
  const addEntry = useStore((s) => s.addEntry);
  const [query, setQuery] = useState("");
  const q = useDeferredValue(query);

  // Searchable text per recipe: what it makes, where, and its id.
  const haystack = useMemo(() => {
    const visible = idx.data.recipes.filter(
      (r) =>
        (showHidden || !r.hidden) &&
        (!hasSave || !onlyUnlocked || u.recipeUnlocked(r)) &&
        // Buildings: placed objects, or named ones like town shops.
        (r.kind === "building" ? r.builds || r.name : r.outputs.length > 0 || (r.inputs.length > 0 && r.stations.length > 0)),
    );
    return visible.map((r) => ({
      r,
      label: n.recipe(r),
      text: fold([n.recipe(r), ...r.outputs.map((o) => n.name(o.item)), ...r.stations.map(n.name), r.id].join(" ")),
    }));
  }, [idx, n, showHidden, hasSave, onlyUnlocked, u]);

  const results = useMemo(() => {
    const words = fold(q).split(/\s+/).filter(Boolean);
    if (!words.length) return [];
    return haystack
      .filter((h) => words.every((w) => h.text.includes(w)))
      .sort((a, b) => {
        // Names starting with the query first, then alphabetical.
        const w = words[0];
        const sa = fold(a.label).startsWith(w) ? 0 : 1;
        const sb = fold(b.label).startsWith(w) ? 0 : 1;
        return sa - sb || a.label.localeCompare(b.label);
      })
      .slice(0, MAX_RESULTS);
  }, [haystack, q]);

  return (
    <section className="panel search">
      <input
        className="search-input"
        type="search"
        placeholder={t.search}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoFocus
      />
      <label className="toggle">
        <input type="checkbox" checked={showHidden} onChange={(e) => setShowHidden(e.target.checked)} />
        {t.showHidden}
      </label>
      {hasSave && (
        <label className="toggle">
          <input type="checkbox" checked={onlyUnlocked} onChange={(e) => setOnlyUnlocked(e.target.checked)} />
          {t.onlyUnlocked}
        </label>
      )}
      <ul className="results">
        {q && !results.length && <li className="muted">{t.noResults}</li>}
        {results.map(({ r, label }) => (
          <li key={r.id}>
            <button className="result" onClick={() => addEntry(r.id)} title={t.add}>
              <RecipeLine r={r} label={label} />
              <span className="add">+</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Lock badge for recipes the followed save hasn't unlocked yet. */
export function LockBadge({ r }: { r: Recipe }) {
  const { t, n, u, hasSave, unlockers } = useGame();
  if (!hasSave || u.recipeUnlocked(r)) return null;
  const techs = (unlockers.get(r.id) ?? []).map((x) => n.name(x.id));
  return (
    <span className="lock-badge" title={techs.length ? fmt(t.unlockedBy, { techs: techs.join(", ") }) : t.locked}>
      🔒
    </span>
  );
}

export function RecipeLine({ r, label }: { r: Recipe; label?: string }) {
  const { t, n } = useGame();
  const out = r.outputs[0];
  const station = n.station(r);
  return (
    <span className="recipe-row">
      <Icon sprite={n.recipeIcon(r)} size={32} />
      <span className="recipe-line">
      <span className="recipe-name">
        <LockBadge r={r} />
        {label ?? n.recipe(r)}
        {out && out.count > 1 && <span className="muted"> ×{out.count}</span>}
        {r.kind === "building" && <span className="badge">{t.building}</span>}
        {r.kind === "craft" && !r.outputs.length && <span className="badge">{t.world}</span>}
        {(r.hidden || (r.kind === "craft" && !r.outputs.length)) && <span className="badge dim">{r.id}</span>}
      </span>
      <span className="recipe-meta">
        {station ? `${t.at} ${station}` : r.kind === "craft" && t.anywhere}
        {r.stations.length > 1 && ` (+${r.stations.length - 1})`}
        <Points points={r.points} />
      </span>
      </span>
    </span>
  );
}
