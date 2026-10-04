import { useDeferredValue, useMemo, useState } from "react";

import { useStore } from "../store";
import type { Recipe } from "../types";
import { Points, useGame } from "./ctx";

const MAX_RESULTS = 120;

/** Strips accents and case so "planche" finds "Planche" and "cle" finds "Clé". */
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function Search() {
  const { t, n, idx } = useGame();
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
        (r.kind === "building" ? r.builds : r.outputs.length > 0 || (r.inputs.length > 0 && r.stations.length > 0)),
    );
    return visible.map((r) => ({
      r,
      label: n.recipe(r),
      text: fold([n.recipe(r), ...r.outputs.map((o) => n.name(o.item)), ...r.stations.map(n.name), r.id].join(" ")),
    }));
  }, [idx, n, showHidden]);

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

export function RecipeLine({ r, label }: { r: Recipe; label?: string }) {
  const { t, n } = useGame();
  const out = r.outputs[0];
  const station = n.station(r);
  return (
    <span className="recipe-line">
      <span className="recipe-name">
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
  );
}
