import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";

import { GameCtx } from "./components/ctx";
import { ListPanel } from "./components/ListPanel";
import { ReportMenu } from "./components/ReportMenu";
import { ResearchTree } from "./components/ResearchTree";
import { Resizer } from "./components/Resizer";
import { Results } from "./components/Results";
import { SaveSelector } from "./components/SaveSelector";
import { Search } from "./components/Search";
import { StockView } from "./components/StockView";
import { UpdateBanner } from "./components/UpdateBanner";
import { fmt, LANGUAGES, strings, type Strings } from "./i18n";
import { namer } from "./names";
import { EMPTY_STOCK, stockOf, unlockedBy, unlocks } from "./progress";
import { DEFAULT_COLUMNS, useStore } from "./store";
import { GAMES } from "./types";
import "./App.css";

const SAVE_POLL_MS = 4000;

/** Game folder in the status bar: change it, or go back to detection. */
function FolderMenu(props: { t: Strings; path?: string; manual: boolean; onChoose(): void; onReset(): void }) {
  const { t, path, manual, onChoose, onReset } = props;
  const [open, setOpen] = useState(false);
  return (
    <span className="folder-menu">
      <button className="link-btn" onClick={() => setOpen((o) => !o)} title={path} aria-expanded={open}>
        {manual && `${t.manualFolder} · `}
        {path ?? t.chooseFolder}
      </button>
      {open && (
        <span className="menu up" role="menu" onMouseLeave={() => setOpen(false)}>
          <button
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onChoose();
            }}
          >
            {t.changeFolder}
          </button>
          {manual && (
            <button
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onReset();
              }}
            >
              {t.resetFolder}
            </button>
          )}
        </span>
      )}
    </span>
  );
}

function useAppVersion(): string | undefined {
  const [version, setVersion] = useState<string>();
  useEffect(() => {
    getVersion().then(setVersion, () => {});
  }, []);
  return version;
}

export default function App() {
  const { ready, game, lang, games, statuses, view, saves, columns, useStock, onlyAvailable } = useStore();
  const { init, selectGame, setLang, loadGame, setGamePath, setView, refreshSaves, setColumns } = useStore.getState();
  const progress = saves[game]?.progress;
  const appVersion = useAppVersion();
  const t = strings(lang);
  const g = games[game];

  useEffect(() => {
    void init();
  }, [init]);

  // Follow the save: poll slot timestamps (cheap) and on window focus.
  useEffect(() => {
    if (!ready) return;
    void refreshSaves(game);
    const timer = setInterval(() => void refreshSaves(game), SAVE_POLL_MS);
    const onFocus = () => void refreshSaves(game);
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [ready, game, refreshSaves]);

  const unlockers = useMemo(() => unlockedBy(g?.index?.data.techs ?? []), [g?.index]);
  const ctx = useMemo(() => {
    if (!g?.index) return null;
    const u = unlocks(progress, g.index.data.techs);
    const idx = g.index;
    const stock = useStock ? stockOf(progress) : EMPTY_STOCK;
    // In stock, or made by a recipe the save has unlocked.
    const obtainable = progress
      ? (item: string) => (stock.total[item] ?? 0) > 0 || (idx.producers.get(item) ?? []).some(u.recipeUnlocked)
      : undefined;
    return {
      t,
      n: namer(g.index.data, lang, t.heavy),
      idx: g.index,
      u,
      hasSave: !!progress,
      available: u.recipeUnlocked,
      unlockers,
      knownMixes: new Set(progress?.lists.knownMixCrafts ?? []),
      stock,
      obtainable,
      strict: onlyAvailable && !!progress,
    };
  }, [g?.index, lang, t, progress, unlockers, useStock, onlyAvailable]);

  const languages = Object.keys(g?.result?.data.locales ?? LANGUAGES).filter((l) => l in LANGUAGES);
  const status = statuses.find((s) => s.game === game);

  const chooseFolder = async () => {
    const dir = await open({ directory: true, title: t.chooseFolder });
    if (typeof dir !== "string") return;
    try {
      await setGamePath(game, dir);
    } catch {
      alert(t.notAGameFolder);
    }
  };

  const warning = g?.result?.warning;
  const warningText =
    warning === "game-not-found" ? t.staleNotFound : warning === "game-updated" ? t.staleUpdated : warning ? t.staleFailed : null;

  return (
    <div className="app">
      <header className="topbar">
        <h1>{t.appTitle}</h1>
        <nav className="games">
          {GAMES.map((id) => (
            <button key={id} className={id === game ? "active" : ""} onClick={() => selectGame(id)}>
              {t[id]}
            </button>
          ))}
        </nav>
        <nav className="views">
          {(["planner", "research", "stock"] as const).map((v) => (
            <button key={v} className={v === view ? "active" : ""} onClick={() => setView(v)}>
              {v === "stock" ? t.stockView : t[v]}
            </button>
          ))}
        </nav>
        <SaveSelector t={t} />
        <ReportMenu t={t} />
        <label className="lang">
          <span className="sr-only">{t.language}</span>
          <select value={lang} onChange={(e) => setLang(e.target.value)}>
            {languages.map((l) => (
              <option key={l} value={l}>
                {LANGUAGES[l]}
              </option>
            ))}
          </select>
        </label>
      </header>

      <UpdateBanner t={t} />

      {warningText && (
        <div className="banner">
          {warningText}
          {warning === "game-not-found" && <button onClick={chooseFolder}>{t.chooseFolder}</button>}
        </div>
      )}

      {!ready || !g || g.status === "loading" ? (
        <main className="center muted">{t.loading}</main>
      ) : g.status === "error" ? (
        <main className="center">
          <div className="setup">
            <p>{g.error?.startsWith("game-not-found") ? fmt(t.gameNotFound, { game: t[game] }) : `${t.error} : ${g.error}`}</p>
            <div className="row">
              <button className="primary" onClick={chooseFolder}>
                {t.chooseFolder}
              </button>
              {status?.manual && <button onClick={() => setGamePath(game, null)}>{t.resetFolder}</button>}
              <button onClick={() => loadGame(game)}>{t.retry}</button>
            </div>
          </div>
        </main>
      ) : ctx ? (
        <GameCtx.Provider value={ctx}>
          {view === "research" ? (
            <main className="layout-research">
              <ResearchTree />
            </main>
          ) : view === "stock" ? (
            <main className="layout-research">
              <StockView />
            </main>
          ) : (
            <main className="layout" style={{ gridTemplateColumns: `${columns[0]}px 8px ${columns[1]}px 8px minmax(320px, 1fr)` }}>
              <Search />
              <Resizer
                width={columns[0]}
                onChange={(w) => setColumns([w, columns[1]])}
                onReset={() => setColumns([DEFAULT_COLUMNS[0], columns[1]])}
              />
              <ListPanel />
              <Resizer
                width={columns[1]}
                onChange={(w) => setColumns([columns[0], w])}
                onReset={() => setColumns([columns[0], DEFAULT_COLUMNS[1]])}
              />
              <Results />
            </main>
          )}
          <footer className="statusbar muted">
            <span className="status-left">
              {g.result?.freshness === "refreshing" ? (
                <span className="refreshing">
                  <span className="spinner" aria-hidden /> {t.refreshing}
                </span>
              ) : g.result?.freshness === "extracted" ? (
                t.extracted
              ) : (
                t.cached
              )}
              {" · "}
              {fmt(t.recipesCount, { n: ctx.idx.data.recipes.length })}
              {ctx.idx.data.gameBuild && ` · ${fmt(t.gameBuild, { build: ctx.idx.data.gameBuild })}`}
              {" · "}
              <FolderMenu
                t={t}
                path={status?.install?.root}
                manual={!!status?.manual}
                onChoose={chooseFolder}
                onReset={() => void setGamePath(game, null)}
              />
            </span>
            {appVersion && <span className="app-version">GK Companion v{appVersion}</span>}
          </footer>
        </GameCtx.Provider>
      ) : null}
    </div>
  );
}
