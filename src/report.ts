// Bug reports and feature requests: GitHub issue forms
// (.github/ISSUE_TEMPLATE), opened prefilled with a diagnostics block.

import type { GameId } from "./types";

export const REPO = "https://github.com/Dragonir44/gk-companion";

export type ReportKind = "bug" | "feature";

const TEMPLATES: Record<ReportKind, { file: string; title: string }> = {
  bug: { file: "bug_report.yml", title: "[Bug] " },
  feature: { file: "feature_request.yml", title: "[Idée] " },
};

/** Mirrors the `diagnostics` command. */
export interface Diagnostics {
  appVersion: string;
  os: string;
  arch: string;
  games: { game: GameId; found: boolean; manual: boolean; buildId?: string | null; saveDirs: number }[];
}

/** What the frontend knows on top: data state per game, language. */
export interface ReportContext {
  diag: Diagnostics;
  lang: string;
  data: Partial<Record<GameId, { freshness?: string; warning?: string; save?: string }>>;
}

const GAME_NAMES: Record<GameId, string> = { gk1: "Graveyard Keeper", gk2: "Graveyard Keeper 2" };

export function diagnosticsText({ diag, lang, data }: ReportContext): string {
  const lines = [`- GK Companion v${diag.appVersion} · ${diag.os} · ${diag.arch} · lang ${lang}`];
  for (const g of diag.games) {
    const d = data[g.game] ?? {};
    const parts = [
      g.found ? (g.buildId ? `build ${g.buildId}` : "found (no Steam build)") : "not found",
      g.manual ? "manual folder" : null,
      d.freshness ? `data ${d.freshness}` : null,
      d.warning ? `warning ${d.warning}` : null,
      `save ${d.save ?? (g.saveDirs ? "not read" : "no save folder")}`,
    ];
    lines.push(`- ${GAME_NAMES[g.game]}: ${parts.filter(Boolean).join(" · ")}`);
  }
  return lines.join("\n");
}

/** Issue form URL; form fields are prefilled through query parameters. */
export function issueUrl(kind: ReportKind, ctx: ReportContext): string {
  const t = TEMPLATES[kind];
  const params = new URLSearchParams({ template: t.file, title: t.title, diagnostics: diagnosticsText(ctx) });
  return `${REPO}/issues/new?${params}`;
}
