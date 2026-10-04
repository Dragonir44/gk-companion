import { useMemo, useState } from "react";

import { fmt } from "../i18n";
import { useStore } from "../store";
import type { Tech } from "../types";
import { Points, useGame } from "./ctx";
import { Icon } from "./Icon";
import { RecipeLine } from "./Search";

const COL = 190;
const ROW = 84;
const NODE_W = 166;
const NODE_H = 60;
const PAD = 16;

/** Tech and every tech it requires, transitively (across branches). */
function ancestors(byId: Map<string, Tech>, id: string): Set<string> {
  const seen = new Set<string>();
  const walk = (t: string) => {
    if (seen.has(t)) return;
    seen.add(t);
    byId.get(t)?.parents.forEach(walk);
  };
  walk(id);
  return seen;
}

function addCost(into: Record<string, number>, cost: Record<string, number>) {
  for (const [k, v] of Object.entries(cost)) into[k] = (into[k] ?? 0) + v;
}

export function ResearchTree() {
  const { t, n, idx } = useGame();
  const showSpoilers = useStore((s) => s.showSpoilers);
  const setShowSpoilers = useStore((s) => s.setShowSpoilers);
  const { techs, branches } = idx.data;
  const byId = useMemo(() => new Map(techs.map((x) => [x.id, x])), [techs]);
  const [branch, setBranch] = useState(branches[0]?.id ?? 0);
  const [selected, setSelected] = useState<string | null>(null);

  const visible = (x: Tech) => showSpoilers || !x.hidden;
  const label = (x: Tech) => {
    if (!visible(x)) return "???";
    if (x.lock) return `🔒 ${n.text(x.lock.name) ?? x.lock.npc} — ${fmt(t.reputation, { value: x.lock.value })}`;
    return n.name(x.id);
  };
  const branchName = (id: number) => {
    const b = branches.find((x) => x.id === id);
    return (b && n.text(b.name)?.replace(/\s+/g, " ")) || `#${id}`;
  };
  const techIcon = (x: Tech) => {
    if (x.icon) return x.icon;
    for (const u of x.unlocks) {
      const r = idx.recipes.get(u);
      const i = r && n.recipeIcon(r);
      if (i) return i;
    }
    return undefined;
  };

  const inBranch = techs.filter((x) => x.branch === branch);
  const width = Math.max(...inBranch.map((x) => x.x), 0) * COL + NODE_W + PAD * 2;
  const height = Math.max(...inBranch.map((x) => x.y), 0) * ROW + NODE_H + PAD * 2;
  const pos = (x: Tech) => ({ left: PAD + x.x * COL, top: PAD + x.y * ROW });

  const path = useMemo(() => (selected ? ancestors(byId, selected) : new Set<string>()), [byId, selected]);
  const sel = selected ? byId.get(selected) : undefined;

  const select = (id: string) => {
    const target = byId.get(id);
    if (target && target.branch !== branch) setBranch(target.branch);
    setSelected(id);
  };

  return (
    <div className="research">
      <section className="panel tree-panel">
        <nav className="tabs branch-tabs">
          {branches.map((b) => (
            <button key={b.id} className={b.id === branch ? "active" : ""} onClick={() => setBranch(b.id)}>
              {branchName(b.id)}
            </button>
          ))}
          <label className="toggle spoilers">
            <input type="checkbox" checked={showSpoilers} onChange={(e) => setShowSpoilers(e.target.checked)} />
            {t.showSpoilers}
          </label>
        </nav>
        <div className="tree-scroll">
          <div className="tree-canvas" style={{ width, height }}>
            <svg width={width} height={height} className="edges">
              {inBranch.flatMap((child) =>
                child.parents
                  .map((p) => byId.get(p))
                  .filter((p): p is Tech => !!p && p.branch === branch)
                  .map((p) => {
                    const a = pos(p);
                    const b = pos(child);
                    const x1 = a.left + NODE_W;
                    const y1 = a.top + NODE_H / 2;
                    const x2 = b.left;
                    const y2 = b.top + NODE_H / 2;
                    const mid = (x1 + x2) / 2;
                    const on = path.has(p.id) && path.has(child.id);
                    return (
                      <path
                        key={`${p.id}>${child.id}`}
                        d={`M${x1},${y1} C${mid},${y1} ${mid},${y2} ${x2},${y2}`}
                        className={on ? "edge on" : "edge"}
                      />
                    );
                  }),
              )}
            </svg>
            {inBranch.map((x) => {
              const external = x.parents.filter((p) => byId.get(p)?.branch !== branch);
              const cls = ["tech", x.lock ? "lock" : "", x.id === selected ? "selected" : path.has(x.id) ? "on-path" : "", visible(x) ? "" : "secret"];
              return (
                <button key={x.id} className={cls.join(" ")} style={pos(x)} onClick={() => select(x.id)}>
                  {visible(x) && <Icon sprite={techIcon(x)} size={28} />}
                  <span className="tech-body">
                    <span className="tech-name">{label(x)}</span>
                    <Points points={x.cost} />
                  </span>
                  {external.length > 0 && (
                    <span className="external" title={external.map((p) => byId.get(p)).filter(Boolean).map((p) => label(p!)).join(", ")}>
                      ⇠{external.length}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </section>
      <TechDetails tech={sel} path={path} byId={byId} label={label} visible={visible} branchName={branchName} onSelect={select} />
    </div>
  );
}

function TechDetails(props: {
  tech?: Tech;
  path: Set<string>;
  byId: Map<string, Tech>;
  label: (t: Tech) => string;
  visible: (t: Tech) => boolean;
  branchName: (id: number) => string;
  onSelect: (id: string) => void;
}) {
  const { tech, path, byId, label, visible, branchName, onSelect } = props;
  const { t, n, idx } = useGame();
  const addEntry = useStore((s) => s.addEntry);

  if (!tech) return <section className="panel tech-details muted center-text">{t.selectTech}</section>;

  const total: Record<string, number> = {};
  for (const id of path) addCost(total, byId.get(id)?.cost ?? {});
  const recipes = visible(tech) ? tech.unlocks.map((u) => idx.recipes.get(u)).filter((r) => r !== undefined) : [];
  const desc = visible(tech) ? n.desc(tech.id) : undefined;

  return (
    <section className="panel tech-details">
      <h2>{label(tech)}</h2>
      <p className="muted small">{branchName(tech.branch)}</p>
      {desc && <p className="desc">{desc}</p>}

      <h3>{t.cost}</h3>
      <Points points={tech.cost} />
      {path.size > 1 && (
        <>
          <h3>{t.totalCost}</h3>
          <Points points={total} />
          <span className="muted small"> ({path.size} {t.techs})</span>
        </>
      )}

      {tech.parents.length > 0 && (
        <>
          <h3>{t.prerequisites}</h3>
          <ul className="links">
            {tech.parents.map((p) => {
              const pt = byId.get(p);
              if (!pt) return null;
              return (
                <li key={p}>
                  <button className="link" onClick={() => onSelect(p)}>
                    {label(pt)}
                  </button>
                  {pt.branch !== tech.branch && <span className="muted small"> — {branchName(pt.branch)}</span>}
                </li>
              );
            })}
          </ul>
        </>
      )}

      {recipes.length > 0 && (
        <>
          <h3>
            {t.unlocks}
            <button className="small-btn" onClick={() => recipes.forEach((r) => addEntry(r.id))}>
              {t.addAll}
            </button>
          </h3>
          <ul className="results">
            {recipes.map((r) => (
              <li key={r.id}>
                <button className="result" onClick={() => addEntry(r.id)} title={t.add}>
                  <RecipeLine r={r} />
                  <span className="add">+</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
