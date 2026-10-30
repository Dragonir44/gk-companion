import { useMemo, useState } from "react";

import { fmt, type Strings } from "../i18n";
import { bestBody, bodyFromItems, emptyBody, fluidOf, OTHER_SLOTS, type Body, type Step, type Totals } from "../morgue";
import { useStore } from "../store";
import type { Recipe } from "../types";
import { fmtNum, useGame } from "./ctx";
import { Icon } from "./Icon";

/** Red and white skull counts, as the game shows them. */
export function Skulls({ s }: { s?: [number, number] }) {
  const [r, w] = s ?? [0, 0];
  return (
    <span className="skulls">
      <span className="skull skull-r">{r}</span>
      <span className="skull skull-w">{w}</span>
    </span>
  );
}

export const talentName = (t: Strings, talent: string) => (t as Record<string, string>)[talent] ?? talent;

/** Mastery a craft needs, named after its station's talent. */
export function MasteryTag({ recipe }: { recipe?: Recipe }) {
  const { t, idx } = useGame();
  if (!recipe?.mastery) return null;
  const talent = idx.data.objects.find((o) => recipe.stations.includes(o.id) && o.work)?.work?.talent;
  return (
    <span className={`mastery ${talent ?? ""}`}>
      {fmt(t.masteryAt, { talent: talent ? talentName(t, talent) : "", n: recipe.mastery })}
    </span>
  );
}

/** Builds a body (from scratch or a save zombie) and shows the best one for the goal. */
export function MorgueView() {
  const { t, n, idx, stock, hasSave, available } = useGame();
  const data = idx.data;
  const m = data.morgue;
  const { goal, body: stored } = useStore((s) => s.morgue);
  const zombies = useStore((s) => s.saves[s.game]?.progress?.zombies) ?? [];
  const setMorgue = useStore((s) => s.setMorgue);
  const [stockOnly, setStockOnly] = useState(true);
  const body = useMemo(() => stored ?? emptyBody(data), [stored, data]);
  const have = stockOnly && hasSave ? stock.total : undefined;
  const plan = useMemo(() => bestBody(data, body, goal, have, available), [data, body, goal, have, available]);
  const items = useMemo(() => new Map(data.items.map((e) => [e.id, e])), [data]);

  if (!m) return <section className="panel muted center-text">{t.morgueNone}</section>;

  const setBody = (b: Body) => setMorgue({ body: b });
  const label = (id: string) => {
    const [r, w] = items.get(id)?.skulls ?? [0, 0];
    return `${n.name(id)} (${r} / ${w})`;
  };
  const used = body.extras.length + body.embalms.length;
  const improved = new Set(plan.steps.flatMap((s) => (s.kind === "improve" ? [s.slot] : [])));

  return (
    <section className="bench">
      <div className="panel bench-form">
        <nav className="tabs">
          {(["red", "white"] as const).map((g) => (
            <button key={g} className={goal === g ? "active" : ""} onClick={() => setMorgue({ goal: g })}>
              {g === "red" ? t.goalRed : t.goalWhite}
            </button>
          ))}
        </nav>
        <p className="muted small">{goal === "red" ? t.goalRedHint : t.goalWhiteHint}</p>

        <div className="bench-row">
          {zombies.length > 0 && (
            <select
              value=""
              onChange={(e) => e.target.value && setBody(bodyFromItems(data, zombies[Number(e.target.value)].items))}
            >
              <option value="">{t.morgueFromZombie}</option>
              {zombies.map((z, i) => (
                <option key={i} value={i}>
                  {n.text(z.name) ?? z.name} — {z.station ? n.name(z.station) : t.zombieFree}
                </option>
              ))}
            </select>
          )}
          <button onClick={() => setBody(emptyBody(data))}>{t.morgueReset}</button>
        </div>

        <h3>
          {t.morgueBody} <span className="muted small">({n.text("ui_skulls_red") ?? "R"} / {n.text("ui_skulls_white") ?? "W"})</span>
        </h3>
        <div className="slots">
          {m.slots.map((slot, i) => (
            <label key={slot} className="slot">
              <Icon sprite={n.icon(body.organs[i] ?? m.organs[i][0])} size={24} />
              <span className="slot-name">{n.name(slot)}</span>
              <select
                value={body.organs[i] ?? ""}
                onChange={(e) => setBody({ ...body, organs: body.organs.map((o, k) => (k === i ? e.target.value || null : o)) })}
              >
                <option value="">{t.morgueEmpty}</option>
                {m.organs[i].map((id) => (
                  <option key={id} value={id}>
                    {label(id)}
                  </option>
                ))}
              </select>
            </label>
          ))}
        </div>

        <h3>
          {t.morgueExtras} <span className="muted small">{fmt(t.embalmsCount, { n: used, max: OTHER_SLOTS })}</span>
        </h3>
        <p className="muted small">{fmt(t.embalmsHint, { max: OTHER_SLOTS })}</p>
        <p className="chips">
          {body.extras.map((id, k) => (
            <button key={`x${k}`} className="chip on" onClick={() => setBody({ ...body, extras: body.extras.filter((_, j) => j !== k) })}>
              {label(id)} ×
            </button>
          ))}
          {body.embalms.map((id, k) => (
            <button key={`e${k}`} className="chip on" onClick={() => setBody({ ...body, embalms: body.embalms.filter((_, j) => j !== k) })}>
              {label(id)} ×
            </button>
          ))}
          {used < OTHER_SLOTS &&
            m.extras.map((id) => (
              <button key={id} className="chip" onClick={() => setBody({ ...body, extras: [...body.extras, id] })}>
                + {label(id)}
              </button>
            ))}
        </p>
        {used < OTHER_SLOTS && (
          <select value="" onChange={(e) => e.target.value && setBody({ ...body, embalms: [...body.embalms, e.target.value] })}>
            <option value="">{t.morgueAddEmbalm}</option>
            {m.embalms
              .filter((id) => !body.embalms.some((e) => fluidOf(e) === fluidOf(id)))
              .map((id) => (
                <option key={id} value={id}>
                  {label(id)}
                </option>
              ))}
          </select>
        )}

        <h3>{n.text("zombie_collar") ?? "Collar"}</h3>
        <select value={body.collar ?? ""} onChange={(e) => setBody({ ...body, collar: e.target.value || undefined })}>
          <option value="">{t.noCollar}</option>
          {m.collars.map((id) => (
            <option key={id} value={id}>
              {n.name(id)} (≤ {items.get(id)?.collar})
            </option>
          ))}
        </select>

        {hasSave && (
          <label className="toggle" title={t.stockOnlyHint}>
            <input type="checkbox" checked={stockOnly} onChange={(e) => setStockOnly(e.target.checked)} />
            {t.stockOnly}
          </label>
        )}
      </div>

      <div className="panel bench-result">
        <table className="mats skull-table">
          <thead>
            <tr>
              <th />
              <th>{n.text("ui_skulls_red") ?? "Red"}</th>
              <th>{n.text("ui_skulls_white") ?? "White"}</th>
            </tr>
          </thead>
          <tbody>
            <TotalsRow label={t.morgueNow} v={plan.before} />
            <TotalsRow label={t.morgueBest} v={plan.after} best />
          </tbody>
        </table>
        {plan.after.red > plan.after.redKept && (
          <p className="note">{fmt(t.capLost, { n: plan.after.red - plan.after.redKept, cap: plan.after.cap ?? 0 })}</p>
        )}

        <h3>{t.morgueSteps}</h3>
        {plan.steps.length === 0 ? (
          <p className="muted">{t.morgueNothing}</p>
        ) : (
          <ol className="steps-list">
            {plan.steps.map((s, i) => (
              <li key={i}>
                <Icon sprite={n.icon(s.item)} size={24} />
                <span>{stepText(t, n.name, s)}</span>
                <Skulls s={items.get(s.item)?.skulls} />
                <MasteryTag recipe={s.recipe} />
                {have && stockShown(s, improved) && (
                  <span className="muted small">
                    {fmtNum(have[stockShown(s, improved)!] ?? 0)} {t.zombieInStock}
                  </span>
                )}
              </li>
            ))}
          </ol>
        )}
      </div>
    </section>
  );
}

/** The stocked item a step takes: the improver for an improvement, nothing for an improved organ. */
function stockShown(s: Step, improved: Set<number>): string | undefined {
  if (s.kind === "improve") return s.recipe.inputs.find((i) => !i.group)?.item;
  if ((s.kind === "swap" || s.kind === "insert") && improved.has(s.slot)) return undefined;
  return s.kind === "remove" ? undefined : s.item;
}

function stepText(t: Strings, name: (id: string) => string, s: Step): string {
  switch (s.kind) {
    case "insert":
      return fmt(t.stepInsert, { item: name(s.item) });
    case "swap":
      return fmt(t.stepSwap, { from: name(s.from), item: name(s.item) });
    case "remove":
      return fmt(t.stepRemove, { item: name(s.item) });
    case "improve":
      return fmt(t.stepImprove, {
        item: name(s.item),
        from: name(s.from),
        improver: name(s.recipe.inputs.find((i) => !i.group)?.item ?? ""),
      });
    case "embalm":
      return fmt(t.stepEmbalm, { item: name(s.item) });
  }
}

function TotalsRow({ label, v, best }: { label: string; v: Totals; best?: boolean }) {
  return (
    <tr className={best ? "best" : ""}>
      <td>{label}</td>
      <td className="qty">
        <span className="skull skull-r">{v.redKept}</span>
        {v.cap !== undefined && <span className="muted small"> / {v.cap}</span>}
      </td>
      <td className="qty">
        <span className="skull skull-w">{v.white}</span>
      </td>
    </tr>
  );
}
