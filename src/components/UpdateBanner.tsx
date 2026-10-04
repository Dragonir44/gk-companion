import { useEffect } from "react";

import { fmt, type Strings } from "../i18n";
import { useUpdater } from "../updater";

export function UpdateBanner({ t }: { t: Strings }) {
  const { update, dismissed, installing, progress, error, check, install, dismiss } = useUpdater();

  useEffect(() => {
    void check(true);
    const onFocus = () => void check();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [check]);

  if (!update || dismissed) return null;
  return (
    <div className="banner update-banner">
      <span>{fmt(t.updateAvailable, { version: update.version })}</span>
      {installing ? (
        <span className="muted">
          {t.updating} {progress !== null && `${Math.round(progress * 100)} %`}
        </span>
      ) : (
        <>
          <button className="primary" onClick={() => void install()}>
            {t.updateInstall}
          </button>
          <button onClick={dismiss}>{t.updateLater}</button>
        </>
      )}
      {error && <span className="error-text">{fmt(t.updateFailed, { error })}</span>}
    </div>
  );
}
