import { useMemo, useState } from "react";

import { bestGear, defencePower, fightPower, readiness, squadsOf, upgrade, type Readiness, type Squad, type Upgrade } from "../army";
import { fmt, type Strings } from "../i18n";
import { useStore } from "../store";
import type { Fight } from "../types";
import { fmtNum, useGame } from "./ctx";
import { Icon } from "./Icon";

const READY_ICON: Record<Readiness, string> = { ready: "✓", gear: "↑", full: "+", short: "✗" };
const readyText = (t: Strings, r: Readiness) =>
  ({ ready: t.armyReady, gear: t.armyReadyGear, full: t.armyReadyFull, short: t.armyShort })[r];

/** Headline numbers, battles by readiness, then squads and what to give them. */
export function ArmyView() {
  const { t, n, idx, stock, obtainable } = useGame();
  const data = idx.data;
  const progress = useStore((s) => s.saves[s.game]?.progress);
  const [fill, setFill] = useState(false);

  const squads = useMemo(() => squadsOf(data, progress?.zombies ?? [], progress?.squads), [data, progress]);
  const best = useMemo(() => bestGear(data, obtainable), [data, obtainable]);
  const up = useMemo(() => upgrade(data, squads, best, stock.total, fill), [data, squads, best, stock, fill]);
  const built = progress?.defences ?? [];

  if (!progress?.zombies || !data.army) return <section className="panel stock-view muted center-text">{t.armyNone}</section>;

  // Headline: every squad sent, every defence counted.
  const all: Fight = { id: "", power: 0, squads: squads.length };
  const defences = defencePower(data, built, all);
  const now = fightPower(data, built, squads.map((s) => s.power), all);
  const geared = fightPower(data, built, up.powers, all);
  const full = fightPower(data, built, up.full, all);
  // Rounds of one battle share its name (the arena): number them.
  const fightName = (f: Fight) => {
    const name = n.text(f.name) ?? f.id;
    const same = data.army!.fights.filter((g) => (n.text(g.name) ?? g.id) === name);
    return same.length > 1 ? `${name} · ${same.indexOf(f) + 1}` : name;
  };

  return (
    <section className="panel army">
      <div className="kpis">
        <Kpi label={t.armyNowPower} value={now} sub={fmt(t.armyNowSplit, { d: defences, s: now - defences })} />
        <Kpi label={t.armyBest} value={geared} delta={geared - now} />
        {up.free > 0 && <Kpi label={t.armyFull} value={full} delta={full - now} sub={fmt(t.armyFreePlaces, { n: up.free })} />}
      </div>
      <p className="muted small">{t.armyRule}</p>

      <div className="army-split">
        <div>
          <h3>{t.armySquads}</h3>
          {squads.length === 0 && <p className="muted">{t.armyNoSquad}</p>}
          <div className="squads">
            {squads.map((s, k) => (
              <SquadCard key={s.place} squad={s} index={k} up={up} />
            ))}
          </div>
        </div>
        <aside>
          <GearPanel up={up} best={best} fill={fill} setFill={setFill} />
          <h3>{t.armyDefences}</h3>
          {built.length === 0 ? <p className="muted">{t.armyNoDefences}</p> : <Defences built={built} />}
        </aside>
      </div>
      <h3>{t.armyFights}</h3>
      <table className="mats fights">
        <thead>
          <tr>
            <th />
            <th>{t.armyNeeded}</th>
            <th>{t.armyYours}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {data.army.fights.map((f) => {
            const p = [squads.map((s) => s.power), up.powers, up.full].map((powers) => fightPower(data, built, powers, f));
            const r = readiness(f.power, p[0], p[1], p[2]);
            const rules = [f.noTowers && t.armyNoTowers, f.noBarricades && t.armyNoBarricades].filter(Boolean).join(", ");
            return (
              <tr key={f.id}>
                <td>
                  {fightName(f)}
                  <span className="muted small">
                    {" "}
                    · {fmt(t.armySquadsMax, { n: f.squads })}
                    {rules && ` · ${rules}`}
                  </span>
                </td>
                <td className="qty">{f.power}</td>
                <td className="qty">{p[0]}</td>
                <td>
                  <span className={`ready ready-${r}`}>
                    <span aria-hidden>{READY_ICON[r]}</span> {r === "short" ? fmt(t.armyShortBy, { n: f.power - p[2] }) : readyText(t, r)}
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>

    </section>
  );
}

function Kpi({ label, value, delta, sub }: { label: string; value: number; delta?: number; sub?: string }) {
  return (
    <div className="kpi">
      <div className="kpi-label">{label}</div>
      <div className="kpi-value">
        {value}
        {!!delta && <span className="kpi-delta">+{delta}</span>}
      </div>
      {sub && <div className="muted small">{sub}</div>}
    </div>
  );
}

/** A gear slot: icon and power, then the upgrade if any. Names in tooltips. */
function GearCell({ cur, next }: { cur?: string; next?: string }) {
  const { t, n, idx } = useGame();
  const power = (id?: string) => (id ? idx.data.items.find((e) => e.id === id)?.gear?.power : undefined);
  const one = (id?: string) =>
    id ? (
      <span className="gear" title={n.name(id)}>
        <Icon sprite={n.icon(id)} size={22} />
        <span className="gear-power">{power(id) ?? 0}</span>
      </span>
    ) : (
      <span className="gear missing" title={t.armyMissing}>
        —
      </span>
    );
  return (
    <span className="gear-cell">
      {one(cur)}
      {next && (
        <>
          <span className="arrow" aria-hidden>
            →
          </span>
          {one(next)}
        </>
      )}
    </span>
  );
}

function SquadCard({ squad: s, index, up }: { squad: Squad; index: number; up: Upgrade }) {
  const { t, n } = useGame();
  const after = up.powers[index];
  return (
    <div className="squad-card">
      <header>
        <strong>{fmt(t.armySquad, { n: index + 1 })}</strong>
        <span className="muted small">{fmt(t.armyPlaces, { n: s.fighters.length, size: s.size })}</span>
        <span className="squad-power">
          {s.power}
          {after > s.power && <span className="kpi-delta"> → {after}</span>}
        </span>
      </header>
      {s.fighters.length === 0 ? (
        <p className="muted small">{fmt(t.armyEmptyWorth, { n: up.full[index] })}</p>
      ) : (
        <table className="fighters">
          <thead>
            <tr>
              <th />
              <th>{t.armyWeapon}</th>
              <th>{t.armyArmor}</th>
            </tr>
          </thead>
          <tbody>
            {s.fighters.map((z, i) => {
              const u = up.fighters.get(z);
              return (
                <tr key={i}>
                  <td className="fighter-name">{n.text(z.name) ?? z.name}</td>
                  <td>
                    <GearCell cur={z.weapon} next={u?.weapon} />
                  </td>
                  <td>
                    <GearCell cur={z.armor} next={u?.armor} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

function GearPanel(props: {
  up: Upgrade;
  best: ReturnType<typeof bestGear>;
  fill: boolean;
  setFill(v: boolean): void;
}) {
  const { up, best, fill, setFill } = props;
  const { t, n, idx } = useGame();
  const { addRecipes, setView } = useStore.getState();
  const allLists = useStore((s) => s.lists);
  const game = useStore((s) => s.game);
  const lists = allLists.filter((l) => l.game === game);
  const [target, setTarget] = useState("");
  const [done, setDone] = useState<string>();
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
    <>
      <h3 title={t.armyBestHint}>{t.armyBestGear}</h3>
      <p className="chips">
        {[best.weapon, best.armor].map((e) =>
          e ? (
            <span key={e.id} className="chip">
              <Icon sprite={n.icon(e.id)} size={16} /> {n.name(e.id)} ({e.gear?.power})
            </span>
          ) : null,
        )}
      </p>
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
        <table className="mats">
          <tbody>
            {up.needs.map((x) => (
              <tr key={x.item}>
                <td className="qty">{fmtNum(x.count)}</td>
                <td>
                  <span className="item">
                    <Icon sprite={n.icon(x.item)} size={20} /> {n.name(x.item)}
                  </span>
                </td>
                <td className="muted small">{x.make > 0 ? fmt(t.armyMake, { n: x.make }) : t.inStock}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {toMake.length > 0 && (
        <div className="bench-row add-row">
          <select value={target} onChange={(e) => setTarget(e.target.value)} aria-label={t.sitesAddTo}>
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
        </div>
      )}
      {done && (
        <p className="muted small">
          {done}{" "}
          <button className="link" onClick={() => setView("planner")}>
            {t.sitesOpenPlanner}
          </button>
        </p>
      )}
    </>
  );
}

function Defences({ built }: { built: string[] }) {
  const { n, idx } = useGame();
  const counts = new Map<string, number>();
  for (const id of built) counts.set(id, (counts.get(id) ?? 0) + 1);
  const power = (id: string) => idx.data.army?.defences.find((d) => d.object === id)?.power ?? 0;
  return (
    <table className="mats">
      <tbody>
        {[...counts].map(([id, count]) => (
          <tr key={id}>
            <td className="qty">{count}</td>
            <td>
              <span className="item">
                <Icon sprite={n.icon(id)} size={20} /> {n.name(id)}
              </span>
            </td>
            <td className="qty">{count * power(id)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
