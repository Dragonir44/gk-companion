import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";

import type { Strings } from "../i18n";
import { issueUrl, type Diagnostics, type ReportContext, type ReportKind } from "../report";
import { useStore } from "../store";
import { GAMES } from "../types";

/** "Report" menu: opens a prefilled GitHub issue form in the browser. */
export function ReportMenu({ t }: { t: Strings }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => ref.current?.contains(e.target as Node) || setOpen(false);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const report = async (kind: ReportKind) => {
    setOpen(false);
    const { lang, games, saves } = useStore.getState();
    const diag = await invoke<Diagnostics>("diagnostics");
    const data: ReportContext["data"] = {};
    for (const g of GAMES) {
      const r = games[g]?.result;
      const save = saves[g];
      data[g] = {
        freshness: r?.freshness,
        warning: r?.warning,
        save: save?.progress ? "followed" : save?.error ? "unreadable" : save?.slots.length ? "not followed" : undefined,
      };
    }
    await openUrl(issueUrl(kind, { diag, lang, data }));
  };

  return (
    <div className="report-menu" ref={ref}>
      <button onClick={() => setOpen((o) => !o)} aria-expanded={open} title={t.reportHint}>
        ⚑ {t.report}
      </button>
      {open && (
        <div className="menu" role="menu">
          <button role="menuitem" onClick={() => void report("bug")}>
            {t.reportBug}
          </button>
          <button role="menuitem" onClick={() => void report("feature")}>
            {t.reportFeature}
          </button>
          <p className="muted small">{t.reportHint}</p>
        </div>
      )}
    </div>
  );
}
