import { convertFileSrc } from "@tauri-apps/api/core";

import { useStore } from "../store";
import { useGame } from "./ctx";

/** URL of an icon sheet: through Tauri's asset protocol, or the dev server in mock mode. */
function sheetUrl(dir: string, file: string): string {
  if ((window as unknown as { __GK_MOCK__?: boolean }).__GK_MOCK__) return `${dir}/${file}`;
  const sep = dir.includes("\\") ? "\\" : "/";
  return convertFileSrc(`${dir}${sep}${file}`);
}

/** A sprite cut from its sheet, scaled to fit a `size` box, pixels kept crisp. */
export function Icon({ sprite, size = 24 }: { sprite?: string; size?: number }) {
  const { idx } = useGame();
  const iconDir = useStore((s) => s.games[s.game]?.result?.iconDir);
  const icons = idx.data.icons;
  const rect = sprite ? icons?.sprites[sprite] : undefined;
  if (!rect || !iconDir) return <span className="icon-box" style={{ width: size, height: size }} aria-hidden />;
  const [sheet, x, y, w, h] = rect;
  const [sw, sh] = icons.sheetSizes[sheet];
  const scale = size / Math.max(w, h);
  return (
    <span className="icon-box" style={{ width: size, height: size }} aria-hidden>
      <span
        className="icon"
        style={{
          width: w * scale,
          height: h * scale,
          backgroundImage: `url("${sheetUrl(iconDir, icons.sheets[sheet])}")`,
          backgroundPosition: `${-x * scale}px ${-y * scale}px`,
          backgroundSize: `${sw * scale}px ${sh * scale}px`,
        }}
      />
    </span>
  );
}
