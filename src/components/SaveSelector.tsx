import { fmt, type Strings } from "../i18n";
import { NO_SAVE, useStore } from "../store";
import type { SaveSlot } from "../types";

function ago(ms: number, t: Strings): string {
  const min = Math.round((Date.now() - ms) / 60000);
  if (min < 1) return t.justNow;
  if (min < 60) return fmt(t.minutesAgo, { n: min });
  const h = Math.round(min / 60);
  if (h < 48) return fmt(t.hoursAgo, { n: h });
  return new Date(ms).toLocaleDateString();
}

function slotLabel(s: SaveSlot, t: Strings, withAge = true): string {
  const day = s.info?.day !== undefined ? ` · ${fmt(t.day, { n: s.info.day })}` : "";
  return `${s.id}${day}${withAge ? ` · ${ago(s.modified, t)}` : ""}`;
}

/** Which save to follow; hidden for games whose saves can't be read yet. */
export function SaveSelector({ t }: { t: Strings }) {
  const game = useStore((s) => s.game);
  const save = useStore((s) => s.saves[s.game]);
  const choice = useStore((s) => s.saveChoice[s.game]);
  const setSaveChoice = useStore((s) => s.setSaveChoice);
  if (!save?.supported) return null;

  const followed = save.progress?.slot;
  return (
    <label className="save-select" title={save.error ? `${t.saveError}: ${save.error}` : undefined}>
      <span className={`save-dot ${followed ? (save.error ? "warn" : "ok") : ""}`} aria-hidden />
      <select value={choice ?? ""} onChange={(e) => setSaveChoice(game, e.target.value || undefined)} aria-label={t.save}>
        <option value="">
          {t.latestSave}
          {save.slots[0] ? ` : ${slotLabel(save.slots[0], t, false)}` : ""}
        </option>
        {save.slots.map((s) => (
          <option key={s.id} value={s.id}>
            {slotLabel(s, t)}
          </option>
        ))}
        <option value={NO_SAVE}>{t.noSave}</option>
      </select>
    </label>
  );
}
