import { useMemo, useState } from "react";

import { chosenRecipe, isProduction, plan, RAW, tree, type Plan, type TreeNode } from "../calc";
import { useActiveList, useStore } from "../store";
import type { CraftList } from "../types";
import { fmtNum, Points, useGame } from "./ctx";
import { Icon } from "./Icon";
import { LockBadge } from "./Search";

type Tab = "materials" | "steps" | "tree";

export function Results() {
  const { t, idx, available } = useGame();
  const list = useActiveList();
  const [tab, setTab] = useState<Tab>("materials");
  const p = useMemo(
    () => (list ? plan(idx, list.entries, list.choices, list.have, available) : undefined),
    [idx, list, available],
  );

  if (!list || !p || !list.entries.length) return <section className="panel results-panel" />;

  return (
    <section className="panel results-panel">
      <nav className="tabs" role="tablist">
        {(["materials", "steps", "tree"] as Tab[]).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "active" : ""} onClick={() => setTab(k)}>
            {t[k]}
          </button>
        ))}
      </nav>
      {tab === "materials" && <Materials p={p} list={list} />}
      {tab === "steps" && <Steps p={p} list={list} />}
      {tab === "tree" && <Tree list={list} />}
    </section>
  );
}

function Materials({ p, list }: { p: Plan; list: CraftList }) {
  const { t, n, idx } = useGame();
  const setHave = useStore((s) => s.setHave);
  const sorted = (m: Map<string, number>) => [...m].sort((a, b) => n.name(a[0]).localeCompare(n.name(b[0])));
  const haveInput = (item: string) => (
    <label>
      {t.owned}
      <input type="number" min={0} value={list.have[item] ?? ""} onChange={(e) => setHave(item, Number(e.target.value))} />
    </label>
  );

  return (
    <div className="tab-body">
      <h3>{t.rawMaterials}</h3>
      {!p.raw.size && !p.groups.size ? (
        <p className="muted">{t.nothingToGather}</p>
      ) : (
        <table className="mats">
          <tbody>
            {sorted(p.raw).map(([item, qty]) => (
              <tr key={item}>
                <td className="qty">{fmtNum(qty)}</td>
                <td>
                  <span className="item">
                    <Icon sprite={n.icon(item)} size={24} />
                    {n.name(item)}
                  </span>
                  {idx.producers.has(item) && <ItemChoice item={item} compact />}
                </td>
                <td className="have">{haveInput(item)}</td>
              </tr>
            ))}
            {sorted(p.groups).map(([g, qty]) => (
              <tr key={g}>
                <td className="qty">{fmtNum(qty)}</td>
                <td colSpan={2}>
                  <span className="item">
                    <Icon sprite={n.icon(g)} size={24} />
                    {n.name(g)}
                  </span>{" "}
                  <span className="badge">{t.anyOf}</span>
                  <div className="muted small">{(idx.data.groups[g] ?? []).map(n.name).join(", ")}</div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {p.used.size > 0 && (
        <>
          <h3>{t.usedFromStock}</h3>
          <table className="mats">
            <tbody>
              {sorted(p.used).map(([item, qty]) => (
                <tr key={item}>
                  <td className="qty">{fmtNum(qty)}</td>
                  <td>
                    <span className="item">
                      <Icon sprite={n.icon(item)} size={24} />
                      {n.name(item)}
                    </span>
                  </td>
                  <td className="have">{haveInput(item)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {p.surplus.size > 0 && (
        <>
          <h3>{t.surplus}</h3>
          <p className="chips">
            {sorted(p.surplus).map(([item, qty]) => (
              <span key={item} className="chip">
                <Icon sprite={n.icon(item)} size={16} />
                {fmtNum(qty)} {n.name(item)}
              </span>
            ))}
          </p>
        </>
      )}
      {Object.keys(p.points).length > 0 && (
        <>
          <h3>{t.techPoints}</h3>
          <Points points={p.points} />
        </>
      )}
      {p.cycles.size > 0 && (
        <p className="muted small">
          {t.cyclesNote} {[...p.cycles].map(n.name).join(", ")}
        </p>
      )}
    </div>
  );
}

function Steps({ p, list }: { p: Plan; list: CraftList }) {
  const { t, n } = useGame();
  const setHave = useStore((s) => s.setHave);
  const steps = p.steps.filter((s) => s.item !== undefined);
  if (!steps.length) return <p className="muted tab-body">{t.stepsEmpty}</p>;
  return (
    <ol className="tab-body steps">
      {steps.map((s, i) => (
        <li key={`${s.recipe.id}-${s.item}-${i}`}>
          <span className="qty">{s.crafts}×</span>
          <span className="item">
            <Icon sprite={n.icon(s.item!)} size={24} />
            <LockBadge r={s.recipe} />
            <strong>{n.name(s.item!)}</strong>
            {s.recipe.outputs.find((o) => o.item === s.item)!.count > 1 && (
              <span className="muted"> (×{s.recipe.outputs.find((o) => o.item === s.item)!.count})</span>
            )}
            <span className="muted"> — {n.station(s.recipe) ?? t.anywhere}</span>
            <Points points={s.recipe.points} />
          </span>
          <ItemChoice item={s.item!} />
          <label className="have">
            {t.owned}
            <input type="number" min={0} value={list.have[s.item!] ?? ""} onChange={(e) => setHave(s.item!, Number(e.target.value))} />
          </label>
        </li>
      ))}
    </ol>
  );
}

function Tree({ list }: { list: CraftList }) {
  const { n, idx, available } = useGame();
  return (
    <div className="tab-body tree">
      {list.entries.map((e) => {
        const r = idx.recipes.get(e.recipe);
        if (!r) return null;
        return (
          <details key={e.recipe} open>
            <summary>
              <span className="qty">{e.count}×</span> <Icon sprite={n.recipeIcon(r)} size={24} /> <strong>{n.recipe(r)}</strong>
            </summary>
            <ul>
              {tree(idx, r, e.count, list.choices, available).map((node, i) => (
                <TreeItem key={i} node={node} />
              ))}
            </ul>
          </details>
        );
      })}
    </div>
  );
}

function TreeItem({ node }: { node: TreeNode }) {
  const { t, n } = useGame();
  const label = (
    <>
      <span className="qty">{fmtNum(node.qty)}</span> <Icon sprite={n.icon(node.item)} size={20} /> {n.name(node.item)}
      {node.group && <span className="badge">{t.anyOf}</span>}
      {node.recipe && (
        <span className="muted">
          {" "}
          — {node.crafts}× {n.station(node.recipe) ?? t.anywhere}
        </span>
      )}
      {node.cycle && <span className="badge dim">↻</span>}
    </>
  );
  if (!node.children.length) return <li className="leaf">{label}</li>;
  return (
    <li>
      <details open>
        <summary>{label}</summary>
        <ul>
          {node.children.map((c, i) => (
            <TreeItem key={i} node={c} />
          ))}
        </ul>
      </details>
    </li>
  );
}

/** Chooses how an item is obtained: one of its recipes, or gathered raw. */
function ItemChoice({ item, compact }: { item: string; compact?: boolean }) {
  const { t, n, idx, available, hasSave } = useGame();
  const list = useActiveList();
  const setChoice = useStore((s) => s.setChoice);
  const producers = idx.producers.get(item) ?? [];
  if (!producers.length || !list) return null;
  const current = chosenRecipe(idx, item, list.choices, available)?.id ?? RAW;
  const label = (r: (typeof producers)[number]) =>
    [n.station(r) ?? t.anywhere, isProduction(r) ? `(${t.production})` : "", r.inputs.map((i) => `${i.count} ${n.name(i.item)}`).join(" + ")]
      .filter(Boolean)
      .join(" ");

  // The default needs no stored choice; storing it would pin it forever.
  const fallback = chosenRecipe(idx, item, {}, available)?.id ?? RAW;
  return (
    <select
      className={compact ? "choice compact" : "choice"}
      value={current}
      onChange={(e) => setChoice(item, e.target.value === fallback ? undefined : e.target.value)}
    >
      <option value={RAW}>{t.gatherIt}</option>
      {producers.map((r) => (
        <option key={r.id} value={r.id}>
          {hasSave && !available(r) ? "🔒 " : ""}
          {t.craftIt}: {label(r)}
        </option>
      ))}
    </select>
  );
}
