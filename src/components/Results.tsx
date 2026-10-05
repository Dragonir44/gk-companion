import { useMemo, useState } from "react";

import { chosenRecipe, isProduction, plan, RAW, tree, type Plan, type TreeNode } from "../calc";
import { useActiveList, useStore } from "../store";
import type { CraftList } from "../types";
import { fmtNum, Points, useGame } from "./ctx";
import { Icon } from "./Icon";
import { MixChoice, runeText } from "./MixChoice";
import { LockBadge } from "./Search";

type Tab = "materials" | "crafting";

export function Results() {
  const { t, idx, available, knownMixes } = useGame();
  const list = useActiveList();
  const [tab, setTab] = useState<Tab>("materials");
  const p = useMemo(
    () => (list ? plan(idx, list.entries, list.choices, { have: list.have, available, knownMixes }) : undefined),
    [idx, list, available, knownMixes],
  );

  if (!list || !p || !list.entries.length) return <section className="panel results-panel" />;

  return (
    <section className="panel results-panel">
      <nav className="tabs" role="tablist">
        {(["materials", "crafting"] as Tab[]).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "active" : ""} onClick={() => setTab(k)}>
            {t[k]}
          </button>
        ))}
      </nav>
      {tab === "materials" && <Materials p={p} list={list} />}
      {tab === "crafting" && <Crafting list={list} />}
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

/** The crafting tree of each list entry, with choices inline. */
function Crafting({ list }: { list: CraftList }) {
  const { t, n, idx, available, knownMixes } = useGame();
  // Bumping the key remounts the tree with every node open or closed.
  const [expand, setExpand] = useState<{ open: boolean; key: number }>({ open: true, key: 0 });
  return (
    <div className="tab-body crafting">
      <div className="crafting-tools">
        <button className="small-btn" onClick={() => setExpand((e) => ({ open: !e.open, key: e.key + 1 }))}>
          {expand.open ? t.collapseAll : t.expandAll}
        </button>
      </div>
      {list.entries.map((e) => {
        const r = idx.recipes.get(e.recipe);
        if (!r) return null;
        return (
          <details key={`${e.recipe}-${expand.key}`} open={expand.open} className="craft-entry">
            <summary className="craft-row root">
              <span className="qty">{e.count}×</span>
              <Icon sprite={n.recipeIcon(r)} size={24} />
              <span className="craft-name">
                <LockBadge r={r} />
                <strong>{n.recipe(r)}</strong>
              </span>
              <span className="craft-where muted">{n.station(r) ?? (r.kind === "craft" ? t.anywhere : "")}</span>
            </summary>
            <ul>
              {r.runes && <MixChoice recipe={r} />}
              {tree(idx, r, e.count, list.choices, { have: list.have, available, knownMixes }).map((node, i) => (
                <CraftNode key={i} node={node} list={list} open={expand.open} />
              ))}
            </ul>
          </details>
        );
      })}
    </div>
  );
}

function CraftNode({ node, list, open }: { node: TreeNode; list: CraftList; open: boolean }) {
  const { t, n, idx } = useGame();
  const setHave = useStore((s) => s.setHave);
  const owned = list.have[node.item] ?? 0;
  const row = (
    <>
      <span className="qty">{fmtNum(node.qty)}</span>
      <Icon sprite={n.icon(node.item)} size={20} />
      <span className="craft-name" title={n.name(node.item)}>
        {node.recipe && <LockBadge r={node.recipe} />}
        {n.name(node.item)}
        {node.group && <span className="badge">{t.anyOf}</span>}
        {node.cycle && <span className="badge dim">↻</span>}
        {owned >= node.qty && <span className="in-stock" title={t.inStock}>✓</span>}
      </span>
      <span className="craft-where muted">
        {node.recipe ? `${node.crafts}× ${n.station(node.recipe) ?? t.anywhere}` : node.group ? "" : t.gathered}
      </span>
      <span className="craft-controls" onClick={(e) => e.preventDefault()}>
        {node.recipe?.runes && <MixChoice recipe={node.recipe} />}
        {!node.group && idx.producers.has(node.item) && <ItemChoice item={node.item} compact />}
        {!node.group && (
          <input
            className="have-input"
            type="number"
            min={0}
            placeholder={t.owned}
            title={t.owned}
            value={list.have[node.item] ?? ""}
            onChange={(e) => setHave(node.item, Number(e.target.value))}
          />
        )}
      </span>
    </>
  );
  if (!node.children.length) return <li className="craft-row leaf">{row}</li>;
  return (
    <li>
      <details open={open}>
        <summary className="craft-row">{row}</summary>
        <ul>
          {node.children.map((c, i) => (
            <CraftNode key={i} node={c} list={list} open={open} />
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
    [
      n.station(r) ?? t.anywhere,
      isProduction(r) ? `(${t.production})` : "",
      r.runes ? runeText(r.runes) : r.inputs.map((i) => `${i.count} ${n.name(i.item)}`).join(" + "),
    ]
      .filter(Boolean)
      .join(" ");

  // The default needs no stored choice; storing it would pin it forever.
  const fallback = chosenRecipe(idx, item, {}, available)?.id ?? RAW;
  const currentRecipe = producers.find((r) => r.id === current);
  return (
    <select
      className={compact ? "choice compact" : "choice"}
      title={currentRecipe ? label(currentRecipe) : t.gatherIt}
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
