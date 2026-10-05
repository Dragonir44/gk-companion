import { useEffect, useMemo, useRef, useState } from "react";

import { chosenMix, mixKey } from "../calc";
import { useActiveList, useStore } from "../store";
import type { Mix, Recipe } from "../types";
import { ownedFor, useGame } from "./ctx";
import { Icon } from "./Icon";

const MARKS = ["🔴", "🟢", "🔵"];
const MAX_SHOWN = 150;

/** "🔴1 🟢5" for rune counts [red, green, blue]. */
export function runeText(runes: [number, number, number]): string {
  return runes.map((n, i) => (n ? `${MARKS[i]}${n}` : "")).filter(Boolean).join(" ");
}

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Picks the ingredient mix of an alchemy recipe among the game's valid ones. */
export function MixChoice({ recipe }: { recipe: Recipe }) {
  const { t, n, idx, knownMixes, stock, obtainable, strict } = useGame();
  const list = useActiveList();
  const setChoice = useStore((s) => s.setChoice);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => ref.current?.contains(e.target as Node) || setOpen(false);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const allMixes = idx.mixes.get(recipe.id) ?? [];
  const have = ownedFor(stock, list?.have ?? {});
  const ctx = { have, knownMixes, obtainable, strict };
  const current = list ? chosenMix(idx, recipe, list.choices, ctx) : undefined;
  const fallback = list ? chosenMix(idx, recipe, {}, ctx) : undefined;
  // "Only what I can make": mixes with an ingredient out of reach are hidden.
  const reachable = strict && obtainable ? allMixes.filter((m) => m.items.every(obtainable)) : allMixes;
  const mixes = reachable.length ? reachable : allMixes;

  const shown = useMemo(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    const text = (m: Mix) => fold(m.items.map((i) => n.name(i)).join(" "));
    return mixes.filter((m) => words.every((w) => text(m).includes(w)));
  }, [mixes, query, n]);

  if (!list || !mixes.length) return null;

  const pick = (m: Mix) => {
    setChoice(mixKey(recipe.id), m.id === fallback?.id ? undefined : m.id);
    setOpen(false);
  };
  const itemsOf = (m: Mix) =>
    m.items.map((i) => (
      <span key={i} className="mix-item" title={n.name(i)}>
        <Icon sprite={n.icon(i)} size={18} />
        <span className="mix-item-name">{n.name(i)}</span>
        {(have[i] ?? 0) > 0 && <span className="in-stock">✓</span>}
        {obtainable && !obtainable(i) && (have[i] ?? 0) === 0 && (
          <span className="unreachable" title={t.unreachable}>
            ✕
          </span>
        )}
      </span>
    ));

  return (
    <span className="mix-choice" ref={ref} onClick={(e) => e.preventDefault()}>
      <button className="mix-current" onClick={() => setOpen((o) => !o)} title={t.mixPick}>
        <span className="runes">{recipe.runes && runeText(recipe.runes)}</span>
        {current?.items.map((i) => <Icon key={i} sprite={n.icon(i)} size={18} />)}
        <span aria-hidden>▾</span>
      </button>
      {open && (
        <span className="menu mix-menu" role="listbox">
          <input autoFocus placeholder={t.mixSearch} value={query} onChange={(e) => setQuery(e.target.value)} />
          <span className="muted small">
            {shown.length} / {mixes.length} {t.mixes}
            {mixes.length < allMixes.length && ` · ${t.mixesHidden}`}
          </span>
          <span className="mix-list">
            {shown.slice(0, MAX_SHOWN).map((m) => (
              <button
                key={m.id}
                role="option"
                aria-selected={m.id === current?.id}
                className={m.id === current?.id ? "mix-row active" : "mix-row"}
                onClick={() => pick(m)}
              >
                {itemsOf(m)}
                {knownMixes.has(m.id) && <span className="badge" title={t.mixKnownHint}>{t.mixKnown}</span>}
              </button>
            ))}
          </span>
        </span>
      )}
    </span>
  );
}
