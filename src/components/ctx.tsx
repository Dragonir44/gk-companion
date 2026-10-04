import { createContext, useContext } from "react";

import type { Available, Index } from "../calc";
import type { Unlocks } from "../progress";
import type { Tech } from "../types";
import type { Strings } from "../i18n";
import type { Namer } from "../names";

export interface Ctx {
  t: Strings;
  n: Namer;
  idx: Index;
  /** Player progress from the followed save (or defaults without one). */
  u: Unlocks;
  /** Whether a save is followed: progress marks are meaningful. */
  hasSave: boolean;
  /** Recipe filter for the planner: unlocked ones when a save is followed. */
  available: Available;
  /** Recipe id -> techs unlocking it. */
  unlockers: Map<string, Tech[]>;
}

export const GameCtx = createContext<Ctx | null>(null);

export function useGame(): Ctx {
  const c = useContext(GameCtx);
  if (!c) throw new Error("useGame outside GameCtx");
  return c;
}

export const fmtNum = (v: number) => (Number.isInteger(v) ? String(v) : v.toFixed(1));

const POINT_CLASS: Record<string, string> = { r: "pt-r", g: "pt-g", b: "pt-b" };

export function Points({ points }: { points?: Record<string, number> }) {
  const entries = Object.entries(points ?? {}).filter(([, v]) => v);
  if (!entries.length) return null;
  return (
    <span className="points">
      {entries.map(([k, v]) => (
        <span key={k} className={`point ${POINT_CLASS[k] ?? ""}`} title={k}>
          {fmtNum(v)}
        </span>
      ))}
    </span>
  );
}
