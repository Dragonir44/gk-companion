import { useMemo, useState } from "react";

import { useStore } from "../store";
import type { Container } from "../types";
import { fmtNum, useGame } from "./ctx";
import { Icon } from "./Icon";

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const PLAYER = "player";

/** Everything stored in the followed save: by item, or by container. */
export function StockView() {
  const { t, n, idx } = useGame();
  const progress = useStore((s) => s.saves[s.game]?.progress);
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"total" | "place">("total");
  const containers = progress?.inventories ?? [];

  const zoneName = (z?: string) => {
    const e = z ? idx.data.zones?.find((x) => x.id === z) : undefined;
    return (e && n.text(e.name)) || z || "";
  };
  const placeName = (c: Container) => (c.object === PLAYER ? t.onMe : n.name(c.object));
  const words = fold(query).split(/\s+/).filter(Boolean);
  const match = (item: string) => words.every((w) => fold(n.name(item)).includes(w));

  const totals = useMemo(() => {
    const m = new Map<string, { total: number; places: { c: Container; count: number }[] }>();
    for (const c of containers)
      for (const [item, count] of Object.entries(c.items)) {
        const e = m.get(item) ?? { total: 0, places: [] };
        e.total += count;
        e.places.push({ c, count });
        m.set(item, e);
      }
    return [...m].sort((a, b) => n.name(a[0]).localeCompare(n.name(b[0])));
  }, [containers, n]);

  if (!containers.length) return <section className="panel stock-view muted center-text">{t.stockNone}</section>;

  return (
    <section className="panel stock-view">
      <header className="stock-header">
        <input className="search-input" type="search" placeholder={t.stockSearch} value={query} onChange={(e) => setQuery(e.target.value)} />
        <nav className="tabs">
          {(["total", "place"] as const).map((k) => (
            <button key={k} className={mode === k ? "active" : ""} onClick={() => setMode(k)}>
              {k === "total" ? t.stockTotal : t.stockByPlace}
            </button>
          ))}
        </nav>
      </header>
      <div className="stock-body">
        {mode === "total" ? (
          <table className="mats">
            <tbody>
              {totals
                .filter(([item]) => match(item))
                .map(([item, { total, places }]) => (
                  <tr key={item}>
                    <td className="qty">{fmtNum(total)}</td>
                    <td>
                      <span className="item">
                        <Icon sprite={n.icon(item)} size={24} />
                        {n.name(item)}
                      </span>
                    </td>
                    <td className="muted small">
                      {places
                        .sort((a, b) => b.count - a.count)
                        .map((p) => `${placeName(p.c)}${p.c.zone ? ` (${zoneName(p.c.zone)})` : ""} ×${p.count}`)
                        .join(" · ")}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        ) : (
          <div className="stock-places">
            {containers
              .map((c) => ({ c, items: Object.entries(c.items).filter(([i]) => match(i)) }))
              .filter((x) => x.items.length)
              .sort((a, b) => zoneName(a.c.zone).localeCompare(zoneName(b.c.zone)))
              .map(({ c, items }, i) => (
                <div key={i} className="stock-place">
                  <h3>
                    {placeName(c)}
                    {c.zone && <span className="muted"> — {zoneName(c.zone)}</span>}
                  </h3>
                  <p className="chips">
                    {items
                      .sort((a, b) => b[1] - a[1])
                      .map(([item, count]) => (
                        <span key={item} className="chip">
                          <Icon sprite={n.icon(item)} size={16} />
                          {fmtNum(count)} {n.name(item)}
                        </span>
                      ))}
                  </p>
                </div>
              ))}
          </div>
        )}
      </div>
    </section>
  );
}
