import { useMemo, useState } from "react";

import { bestGear, fightPower, squadsOf, upgrade } from "../army";
import { fmt } from "../i18n";
import { useStore } from "../store";
import type { Entity } from "../types";
import { fmtNum, useGame } from "./ctx";
import { Icon } from "./Icon";

/** Squads, their gear and power, the best gear to give them, and the battles. */
export function ArmyView() {
  const { t, n, idx, stock, obtainable } = useGame();
  const data = idx.data;
  const progress = useStore((s) => s.saves[s.game]?.progress);
  const { addRecipes, setView } = useStore.getState();
  const allLists = useStore((s) => s.lists);
  const game = useStore((s) => s.game);
  const lists = allLists.filter((l) => l.game === game);
  const [target, setTarget] = useState("");
  const [done, setDone] = useState<string>();
  const [fill, setFill] = useState(false);

  const items = useMemo(() => new Map(data.items.map((e) => [e.id, e])), [data]);
  const squads = useMemo(() => squadsOf(data, progress?.zombies ?? [], progress?.squads), [data, progress]);
  const best = useMemo(() => bestGear(data, obtainable), [data, obtainable]);
  const up = useMemo(() => upgrade(data, squads, best, stock.total, fill), [data, squads, best, stock, fill]);
  const built = progress?.defences ?? [];

  if (!progress?.zombies || !data.army) return <section className="panel stock-view muted center-text">{t.armyNone}</section>;

  const defenceCounts = new Map<string, number>();
  for (const id of built) defenceCounts.set(id, (defenceCounts.get(id) ?? 0) + 1);
  const defencePower = (id: string) => data.army?.defences.find((d) => d.object === id)?.power ?? 0;
  const gearLabel = (id?: string) => {
    const e = id ? items.get(id) : undefined;
    return e ? `${n.name(e.id)} (${e.gear?.power ?? 0})` : "—";
  };
  const toMake = up.needs.filter((x) => x.make > 0);
  const addToList = () => {
    const recipes = toMake.flatMap((x) => {
      const r = idx.producers.get(x.item)?.[0];
      return r ? Array(x.make).fill(r.id) : [];
    });
    addRecipes(target || null, recipes, t.armyListName);
    setDone(fmt(t.sitesAdded, { n: recipes.length }));
  };

  return (
    <section className="panel stock-view army">
      <p className="muted small">{t.armyRule}</p>

      <h3>{t.armyBestGear}</h3>
      <p className="chips" title={t.armyBestHint}>
        {[best.weapon, best.armor].map((e: Entity | undefined, i) =>
          e ? (
            <span key={i} className="chip">
              <Icon sprite={n.icon(e.id)} size={16} /> {gearLabel(e.id)}
            </span>
          ) : null,
        )}
      </p>

      <h3>{t.armySquads}</h3>
      {squads.length === 0 && <p className="muted">{t.armyNoSquad}</p>}
      {squads.map((s, k) => (
        <div key={s.place} className="squad">
          <h4>
            {fmt(t.armySquad, { n: k + 1 })} · {fmt(t.armyPlaces, { n: s.fighters.length, size: s.size })} · {t.armyPower} {s.power}
            {up.powers[k] > s.power && <span className="note"> → {up.powers[k]}</span>}
            {up.full[k] > up.powers[k] && <span className="muted"> ({fmt(t.armyIfFull, { n: up.full[k] })})</span>}
          </h4>
          {s.fighters.length === 0 && <p className="muted small">{t.armyEmpty}</p>}
          <table className="mats">
            <tbody>
              {s.fighters.map((z, i) => {
                const u = up.fighters.get(z);
                const own = (z.weapon && z.armor && items.get(z.weapon)?.gear && items.get(z.armor)?.gear) ? null : t.armyMissing;
                return (
                  <tr key={i}>
                    <td>{n.text(z.name) ?? z.name}</td>
                    <td>
                      <span className="item">
                        <Icon sprite={n.icon(z.weapon ?? "")} size={20} />
                        {gearLabel(z.weapon)}
                        {u?.weapon && <span className="note">→ {gearLabel(u.weapon)}</span>}
                      </span>
                    </td>
                    <td>
                      <span className="item">
                        <Icon sprite={n.icon(z.armor ?? "")} size={20} />
                        {gearLabel(z.armor)}
                        {u?.armor && <span className="note">→ {gearLabel(u.armor)}</span>}
                      </span>
                    </td>
                    <td className="muted small">{own}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ))}

      {squads.length > 0 && (
        <>
          <h3>{t.armyNeeds}</h3>
          {up.free > 0 && (
            <label className="toggle">
              <input type="checkbox" checked={fill} onChange={(e) => setFill(e.target.checked)} />
              {fmt(t.armyFill, { n: up.free })}
            </label>
          )}
          {up.needs.length === 0 ? (
            <p className="muted">{t.armyAllGood}</p>
          ) : (
            <>
              <p className="chips">
                {up.needs.map((x) => (
                  <span key={x.item} className="chip">
                    <Icon sprite={n.icon(x.item)} size={16} /> {fmtNum(x.count)} {n.name(x.item)}
                    {x.make > 0 && <span className="muted"> · {fmt(t.armyMake, { n: x.make })}</span>}
                  </span>
                ))}
              </p>
              {toMake.length > 0 && (
                <p className="bench-row">
                  {t.sitesAddTo}{" "}
                  <select value={target} onChange={(e) => setTarget(e.target.value)}>
                    {lists.map((l) => (
                      <option key={l.id} value={l.id}>
                        {l.name || t.defaultListName}
                      </option>
                    ))}
                    <option value="">{t.newList}</option>
                  </select>
                  <button className="primary" onClick={addToList}>
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
                </p>
              )}
            </>
          )}
        </>
      )}

      <h3>{t.armyDefences}</h3>
      {built.length === 0 ? (
        <p className="muted">{t.armyNoDefences}</p>
      ) : (
        <p className="chips">
          {[...defenceCounts].map(([id, count]) => (
            <span key={id} className="chip">
              <Icon sprite={n.icon(id)} size={16} /> {count} × {n.name(id)} ({defencePower(id)})
            </span>
          ))}
        </p>
      )}

      <h3>{t.armyFights}</h3>
      <table className="mats fights">
        <thead>
          <tr>
            <th />
            <th>{t.armyRequired}</th>
            <th>{t.armyNow}</th>
            <th>{t.armyBest}</th>
            {up.free > 0 && <th>{t.armyFull}</th>}
          </tr>
        </thead>
        <tbody>
          {data.army.fights.map((f) => {
            const now = fightPower(data, built, squads.map((s) => s.power), f);
            const after = fightPower(data, built, up.powers, f);
            const full = fightPower(data, built, up.full, f);
            const rules = [f.noTowers && t.armyNoTowers, f.noBarricades && t.armyNoBarricades].filter(Boolean).join(", ");
            return (
              <tr key={f.id}>
                <td>
                  {n.text(f.name) ?? f.id}
                  <span className="muted small">
                    {" "}
                    · {fmt(t.armySquadsMax, { n: f.squads })}
                    {rules && ` · ${rules}`}
                  </span>
                </td>
                <td className="qty">{f.power}</td>
                <td className={`qty ${now >= f.power ? "ok" : "short"}`}>{now}</td>
                <td className={`qty ${after >= f.power ? "ok" : "short"}`}>{after}</td>
                {up.free > 0 && <td className={`qty ${full >= f.power ? "ok" : "short"}`}>{full}</td>}
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
