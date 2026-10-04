// In-app updates: checks the latest GitHub release (latest.json, signed by
// the release workflow) and installs it in place, then restarts.

import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";

/** Re-check on window focus at most this often. */
const RECHECK_MS = 60 * 60 * 1000;

interface UpdateState {
  update: Update | null;
  dismissed: boolean;
  installing: boolean;
  /** 0..1, or null while the size is unknown. */
  progress: number | null;
  error: string | null;
  lastCheck: number;
  check(force?: boolean): Promise<void>;
  install(): Promise<void>;
  dismiss(): void;
}

export const useUpdater = create<UpdateState>((set, get) => ({
  update: null,
  dismissed: false,
  installing: false,
  progress: null,
  error: null,
  lastCheck: 0,

  async check(force = false) {
    const s = get();
    if (s.installing || (!force && Date.now() - s.lastCheck < RECHECK_MS)) return;
    set({ lastCheck: Date.now() });
    try {
      const update = await check();
      if (update && update.version !== s.update?.version) set({ update, dismissed: false, error: null });
    } catch (e) {
      // Offline, rate-limited, or a platform/package without updates
      // (deb, rpm, dev builds): nothing to show.
      console.info("update check:", e);
    }
  },

  async install() {
    const update = get().update;
    if (!update) return;
    set({ installing: true, progress: null, error: null });
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          done += event.data.chunkLength;
          set({ progress: total ? done / total : null });
        }
      });
      await relaunch();
    } catch (e) {
      set({ installing: false, error: String(e) });
    }
  },

  dismiss() {
    set({ dismissed: true });
  },
}));
