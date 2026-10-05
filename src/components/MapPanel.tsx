import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { useStore } from "../store";
import type { MapPoint } from "../worldmap";
import { useGame } from "./ctx";
import { sheetUrl } from "./Icon";
import { useDragScroll } from "./useDragScroll";

export interface Marker {
  key: string;
  at: MapPoint;
  label: string;
  title: string;
  selected?: boolean;
}

export interface Landmark {
  at: MapPoint;
  title: string;
}

const MAX_ZOOM = 4;
const WHEEL_STEP = 1.2;
/** Space kept around the markers when framing them. */
const FRAME_MARGIN = 0.06;

/**
 * The game's map with markers on it: drag to pan, wheel to zoom around the
 * pointer. Opens framed on the markers.
 */
export function MapPanel({
  markers,
  landmarks,
  focused,
  hovered,
  onPick,
}: {
  markers: Marker[];
  landmarks: Landmark[];
  /** Marker to bring into view and highlight. */
  focused?: string;
  /** Marker to highlight only. */
  hovered?: string;
  onPick: (key: string) => void;
}) {
  const { t, idx } = useGame();
  const iconDir = useStore((s) => s.games[s.game]?.result?.iconDir);
  const map = idx.data.map!;
  const rect = idx.data.icons.sprites[map.sprite];
  const [w, h] = rect ? [rect[3], rect[4]] : [1, 1];
  const scrollRef = useDragScroll<HTMLDivElement>({ sidewaysWheel: false });
  const [zoom, setZoom] = useState(0);
  const zoomRef = useRef(0);
  zoomRef.current = zoom;
  // Content point to keep under a viewport point once `zoom` applies.
  const anchor = useRef<{ zoom: number; x: number; y: number; vx: number; vy: number } | null>(null);

  const fitZoom = useCallback(() => {
    const el = scrollRef.current;
    return el ? Math.min(el.clientWidth / w, el.clientHeight / h) : 1;
  }, [scrollRef, w, h]);

  /** Zoom and scroll so the [u0, v0]..[u1, v1] box fills the view. */
  const frame = useCallback(
    (u0: number, v0: number, u1: number, v1: number) => {
      const el = scrollRef.current;
      if (!el) return;
      const bw = Math.max(u1 - u0 + 2 * FRAME_MARGIN, 0.15) * w;
      const bh = Math.max(v1 - v0 + 2 * FRAME_MARGIN, 0.15) * h;
      const z = Math.min(MAX_ZOOM, Math.max(fitZoom(), Math.min(el.clientWidth / bw, el.clientHeight / bh)));
      anchor.current = { zoom: z, x: ((u0 + u1) / 2) * w, y: ((v0 + v1) / 2) * h, vx: el.clientWidth / 2, vy: el.clientHeight / 2 };
      setZoom(z);
      // Same zoom: no re-render, scroll now.
      if (z === zoomRef.current) {
        el.scrollLeft = anchor.current.x * z - anchor.current.vx;
        el.scrollTop = anchor.current.y * z - anchor.current.vy;
        anchor.current = null;
      }
    },
    [scrollRef, w, h, fitZoom],
  );

  const frameMarkers = useCallback(() => {
    if (!markers.length) return frame(0, 0, 1, 1);
    const us = markers.map((m) => m.at[0]);
    const vs = markers.map((m) => m.at[1]);
    frame(Math.min(...us), Math.min(...vs), Math.max(...us), Math.max(...vs));
  }, [markers, frame]);

  // Frame the markers once the panel has its size.
  const framed = useRef(false);
  useEffect(() => {
    if (framed.current || !scrollRef.current?.clientWidth) return;
    framed.current = true;
    frameMarkers();
  }, [frameMarkers, scrollRef]);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    const a = anchor.current;
    if (!el || !a || a.zoom !== zoom) return;
    el.scrollLeft = a.x * zoom - a.vx;
    el.scrollTop = a.y * zoom - a.vy;
    anchor.current = null;
  }, [zoom, scrollRef]);

  // Wheel zooms around the pointer.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      const vx = e.clientX - r.left;
      const vy = e.clientY - r.top;
      const z = zoomRef.current;
      if (!z) return;
      const next = Math.min(MAX_ZOOM, Math.max(fitZoom(), e.deltaY < 0 ? z * WHEEL_STEP : z / WHEEL_STEP));
      if (next === z) return;
      anchor.current = { zoom: next, x: (el.scrollLeft + vx) / z, y: (el.scrollTop + vy) / z, vx, vy };
      setZoom(next);
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, [scrollRef, fitZoom]);

  // Bring the focused marker into view.
  useEffect(() => {
    const el = scrollRef.current;
    const m = markers.find((x) => x.key === focused);
    if (!el || !m || !zoom) return;
    const x = m.at[0] * w * zoom;
    const y = m.at[1] * h * zoom;
    const inside = x > el.scrollLeft && x < el.scrollLeft + el.clientWidth && y > el.scrollTop && y < el.scrollTop + el.clientHeight;
    if (!inside) el.scrollTo({ left: x - el.clientWidth / 2, top: y - el.clientHeight / 2, behavior: "smooth" });
    // Only when the focus changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focused]);

  const z = zoom || 1;
  return (
    <div className="map-panel">
      <div className="map-scroll" ref={scrollRef}>
        <div className="map-canvas" style={{ width: w * z, height: h * z, visibility: zoom ? "visible" : "hidden" }}>
          {iconDir && rect && (
            <img
              className="map-image"
              src={sheetUrl(iconDir, idx.data.icons.sheets[rect[0]])}
              width={w * z}
              height={h * z}
              alt=""
              draggable={false}
            />
          )}
          {landmarks.map((l, i) => (
            <span key={i} className="map-landmark" title={l.title} style={{ left: l.at[0] * w * z, top: l.at[1] * h * z }} />
          ))}
          {markers.map((m) => (
            <button
              key={m.key}
              className={`map-marker${m.selected ? " on" : ""}${m.key === focused || m.key === hovered ? " focus" : ""}`}
              style={{ left: m.at[0] * w * z, top: m.at[1] * h * z }}
              title={m.title}
              onClick={() => onPick(m.key)}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>
      <div className="map-tools">
        <button onClick={frameMarkers} title={t.mapFrameSites}>
          ⌖
        </button>
        <button onClick={() => frame(0, 0, 1, 1)} title={t.mapWhole}>
          ⤢
        </button>
      </div>
    </div>
  );
}
