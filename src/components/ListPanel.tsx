import { useState } from "react";

import { fmt } from "../i18n";
import { useActiveList, useStore } from "../store";
import { useGame } from "./ctx";
import { RecipeLine } from "./Search";

export function ListPanel() {
  const { t, idx } = useGame();
  const game = useStore((s) => s.game);
  const lists = useStore((s) => s.lists).filter((l) => l.game === game);
  const active = useActiveList();
  const { createList, selectList, renameList, deleteList, setCount, removeEntry } = useStore.getState();
  const [renaming, setRenaming] = useState<string | null>(null);

  const listName = (name: string) => name || t.defaultListName;

  return (
    <section className="panel list">
      <header className="list-header">
        {renaming !== null && active ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              renameList(active.id, renaming.trim());
              setRenaming(null);
            }}
          >
            <input autoFocus value={renaming} onChange={(e) => setRenaming(e.target.value)} onBlur={(e) => e.currentTarget.form?.requestSubmit()} />
          </form>
        ) : (
          <select value={active?.id ?? ""} onChange={(e) => selectList(e.target.value)} aria-label={t.lists}>
            {!active && <option value="">{t.defaultListName}</option>}
            {lists.map((l) => (
              <option key={l.id} value={l.id}>
                {listName(l.name)} ({l.entries.length})
              </option>
            ))}
          </select>
        )}
        <div className="list-actions">
          <button onClick={() => createList(`${t.defaultListName} ${lists.length + 1}`)} title={t.newList}>
            ＋
          </button>
          {active && (
            <>
              <button onClick={() => setRenaming(active.name)} title={t.rename}>
                ✎
              </button>
              <button
                onClick={() => confirm(fmt(t.confirmDelete, { name: listName(active.name) })) && deleteList(active.id)}
                title={t.delete}
              >
                🗑
              </button>
            </>
          )}
        </div>
      </header>

      {!active?.entries.length ? (
        <p className="muted empty">{t.emptyList}</p>
      ) : (
        <ul className="entries">
          {active.entries.map((e, i) => {
            const r = idx.recipes.get(e.recipe);
            return (
              <li key={e.recipe} className="entry">
                <input
                  type="number"
                  min={1}
                  value={e.count}
                  onChange={(ev) => setCount(i, Number(ev.target.value))}
                  aria-label={t.times}
                />
                <span className="muted">×</span>
                {r ? <RecipeLine r={r} /> : <span className="muted">{e.recipe}</span>}
                <button className="icon" onClick={() => removeEntry(i)} title={t.remove}>
                  ✕
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
