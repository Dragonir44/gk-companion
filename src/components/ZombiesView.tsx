import { useMemo, useState } from "react";

import { fmt } from "../i18n";
import { bodyFromItems, totals } from "../morgue";
import { useStore } from "../store";
import type { Recipe, TalentNode } from "../types";
import { collarFor, toolsFor, zombiePlans, type Option } from "../zombies";
import { fmtNum, Points, useGame } from "./ctx";
import { Icon } from "./Icon";
import { talentName } from "./MorgueView";

const NEW = "new";

/** What a zombie must learn for a job: per tool, the tree nodes to buy. */
export function ZombiesView() {
  const { t, n, idx, stock, hasSave, available } = useGame();
  const data = idx.data;
  const zombies = useStore((s) => s.saves[s.game]?.progress?.zombies) ?? [];
  const { setMorgue, setView } = useStore.getState();
  const [who, setWho] = useState(NEW);
  const z = who === NEW ? undefined : zombies[Number(who)];

  const crafts = useMemo(() => {
    const m = new Map<string, Recipe[]>();
    for (const r of data.recipes)
      if (r.kind === "craft" && !r.hidden && r.mastery) for (const s of r.stations) m.set(s, [...(m.get(s) ?? []), r]);
    return m;
  }, [data]);
  const stations = useMemo(
    () => data.objects.filter((o) => o.work?.zombie && crafts.has(o.id)).sort((a, b) => n.name(a.id).localeCompare(n.name(b.id))),
    [data, crafts, n],
  );
  const [stationId, setStationId] = useState<string>();
  const station = stations.find((s) => s.id === (stationId ?? z?.station)) ?? stations[0];
  const atStation = useMemo(
    () => (station ? (crafts.get(station.id) ?? []).slice().sort((a, b) => (a.mastery ?? 0) - (b.mastery ?? 0)) : []),
    [station, crafts],
  );
  // Crafts left out of the job, per station; by default the locked ones.
  const [off, setOff] = useState<Record<string, string[]>>({});
  const excluded = useMemo(
    () => new Set(station ? (off[station.id] ?? (hasSave ? atStation.filter((r) => !available(r)).map((r) => r.id) : [])) : []),
    [station, off, hasSave, atStation, available],
  );
  const job = useMemo(() => atStation.filter((r) => !excluded.has(r.id)), [atStation, excluded]);
  const setExcluded = (ids: string[]) => station && setOff({ ...off, [station.id]: ids });
  const [speed, setSpeed] = useState(false);
  const [picked, setPicked] = useState<string>();

  const tools = useMemo(() => (station ? toolsFor(data, station) : []), [data, station]);
  const studied = z?.studied;
  const options = useMemo(
    () => (station && job.length ? zombiePlans(data, { station, crafts: job }, { studied: studied ?? [] }, tools, speed) : []),
    [data, station, job, studied, tools, speed],
  );
  // Default: the best option with a tool at hand (held, in stock, or none).
  const atHand = (o: Option) => !o.tool || o.tool === z?.tool || (stock.total[o.tool] ?? 0) > 0;
  const chosen =
    options.find((o) => (o.tool ?? "") === picked) ?? (hasSave ? options.find((o) => o.reached && atHand(o)) : undefined) ?? options[0];

  if (!data.zombieTree?.length) return <section className="panel muted center-text">{t.zombiesNone}</section>;

  const zombieName = (i: number) => {
    const zz = zombies[i];
    return `${n.text(zz.name) ?? zz.name} — ${zz.station ? n.name(zz.station) : t.zombieFree}`;
  };
  const need = Math.max(0, ...job.map((r) => r.mastery ?? 0));
  const toolNote = (o: Option) =>
    o.tool && z?.tool === o.tool ? t.zombieHolds : o.tool && hasSave && (stock.total[o.tool] ?? 0) > 0 ? t.zombieInStock : "";

  return (
    <section className="bench">
      <div className="panel bench-form">
        <label className="field">
          <span>{t.zombieWho}</span>
          <select
            value={who}
            onChange={(e) => {
              setWho(e.target.value);
              setStationId(undefined);
              setPicked(undefined);
            }}
          >
            <option value={NEW}>{t.zombieNew}</option>
            {zombies.map((_, i) => (
              <option key={i} value={i}>
                {zombieName(i)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t.zombieStation}</span>
          <select
            value={station?.id ?? ""}
            onChange={(e) => {
              setStationId(e.target.value);
              setPicked(undefined);
            }}
          >
            {stations.map((s) => (
              <option key={s.id} value={s.id}>
                {n.name(s.id)} — {talentName(t, s.work!.talent)}
              </option>
            ))}
          </select>
        </label>
        <label className="toggle">
          <input type="checkbox" checked={speed} onChange={(e) => setSpeed(e.target.checked)} />
          {t.zombieSpeed}
        </label>

        <h3>
          {t.zombieCrafts}{" "}
          <button className="link-btn" onClick={() => setExcluded([])}>
            {t.zombieAll}
          </button>{" "}
          <button className="link-btn" onClick={() => setExcluded(atStation.map((r) => r.id))}>
            {t.zombieNoneSel}
          </button>
        </h3>
        <ul className="craft-picks">
          {atStation.map((r) => (
            <li key={r.id} className={hasSave && !available(r) ? "muted" : ""}>
              <label>
                <input
                  type="checkbox"
                  checked={!excluded.has(r.id)}
                  onChange={(e) =>
                    setExcluded(e.target.checked ? [...excluded].filter((id) => id !== r.id) : [...excluded, r.id])
                  }
                />
                <Icon sprite={n.icon(r.outputs[0]?.item ?? r.id) ?? r.icon} size={20} />
                <span className="grow">{n.recipe(r)}</span>
                <span className="qty">{r.mastery}</span>
              </label>
            </li>
          ))}
        </ul>
      </div>

      <div className="panel bench-result">
        {!station || !job.length ? (
          <p className="muted">{t.zombieNoneSel}</p>
        ) : (
          <>
            <h3>
              {fmt(t.zombieNeed, { n: need })} · {talentName(t, station.work!.talent)}
            </h3>
            <table className="mats options">
              <thead>
                <tr>
                  <th>{t.zombieOptions}</th>
                  <th>{t.zombieNodes}</th>
                  <th>{t.zombieSkulls}</th>
                  <th>{t.zombiePoints}</th>
                </tr>
              </thead>
              <tbody>
                {options.map((o) => (
                  <tr
                    key={o.tool ?? ""}
                    className={`${o === chosen ? "active" : ""} ${o.reached ? "" : "out"}`}
                    onClick={() => setPicked(o.tool ?? "")}
                  >
                    <td>
                      <span className="item">
                        {o.tool ? <Icon sprite={n.icon(o.tool)} size={20} /> : null}
                        {o.tool ? `${n.name(o.tool)} (+${o.toolBonus})` : t.zombieNoTool}
                        <span className="muted small">{toolNote(o)}</span>
                      </span>
                    </td>
                    <td className="qty">{o.reached ? o.nodes.length : t.zombieOut}</td>
                    <td className="qty">{o.reached ? o.skulls : ""}</td>
                    <td>{o.reached && <Points points={o.cost} />}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {chosen && <Details option={chosen} zombie={z} />}
            {z && (
              <button
                onClick={() => {
                  setMorgue({ goal: "red", body: bodyFromItems(data, z.items) });
                  setView("morgue");
                }}
              >
                {t.toMorgue}
              </button>
            )}
          </>
        )}
      </div>
    </section>
  );
}

function Details({ option: o, zombie }: { option: Option; zombie?: { items: string[]; collar?: string; points: Record<string, number> } }) {
  const { t, n, idx } = useGame();
  const data = idx.data;
  if (!o.reached) return <p className="note">{t.zombieOutHint}</p>;
  const cap = zombie?.collar ? data.items.find((i) => i.id === zombie.collar)?.collar : undefined;
  const red = zombie ? totals(data, bodyFromItems(data, zombie.items)).red : undefined;
  const collar = cap !== undefined && cap >= o.skulls ? undefined : collarFor(data, o.skulls);
  const needPoints = Object.entries(o.cost);
  return (
    <>
      <h3>{t.zombieNodes}</h3>
      {o.nodes.length === 0 ? (
        <p className="muted">{t.zombieNothing}</p>
      ) : (
        <ol className="steps-list">
          {o.nodes.map((node) => (
            <NodeLine key={node.id} node={node} />
          ))}
        </ol>
      )}
      {o.skulls > 0 && (
        <>
          <p>
            {cap !== undefined && cap >= o.skulls
              ? fmt(t.zombieCollarOk, { collar: n.name(zombie!.collar!), n: o.skulls })
              : collar
                ? fmt(t.zombieCollarNeed, { n: o.skulls, collar: n.name(collar.id) })
                : fmt(t.zombieCollarNone, { n: o.skulls })}
          </p>
          {red !== undefined && (
            <p className={red >= o.skulls ? "muted" : "note"}>{fmt(red >= o.skulls ? t.zombieBodyOk : t.zombieBodyShort, { red, n: o.skulls })}</p>
          )}
        </>
      )}
      {zombie &&
        needPoints.map(([k, v]) => (
          <p key={k} className={(zombie.points[k] ?? 0) >= v ? "muted" : "note"}>
            <Points points={{ [k]: v }} /> {fmt(t.zombiePointsHave, { have: fmtNum(zombie.points[k] ?? 0), need: fmtNum(v) })}
          </p>
        ))}
    </>
  );
}

function NodeLine({ node }: { node: TalentNode }) {
  const { t, n } = useGame();
  const effects = [
    node.mastery ? fmt(t.zombieLevel, { n: node.mastery }) : "",
    node.masteryBonus ? fmt(t.zombieMasteryBonus, { n: node.masteryBonus }) : "",
    node.speedBonus ? fmt(t.zombieSpeedBonus, { n: node.speedBonus }) : "",
  ].filter(Boolean);
  return (
    <li title={n.text(node.desc)}>
      <Icon sprite={node.icon} size={24} />
      <span>
        {n.text(node.name) ?? talentName(t, node.talent)}
        <span className="muted small"> · {talentName(t, node.talent)}</span>
      </span>
      <span className="small">{effects.join(" · ")}</span>
      <Points points={node.cost} />
    </li>
  );
}
