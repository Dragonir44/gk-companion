import { useRef } from "react";

/**
 * Vertical drag handle between two panels. Reports the new width of the
 * panel on its left; double-click resets it.
 */
export function Resizer({ width, onChange, onReset, min = 220, max = 900 }: {
  width: number;
  onChange(w: number): void;
  onReset(): void;
  min?: number;
  max?: number;
}) {
  const start = useRef<{ x: number; w: number } | null>(null);
  return (
    <div
      className="resizer"
      role="separator"
      aria-orientation="vertical"
      onPointerDown={(e) => {
        start.current = { x: e.clientX, w: width };
        e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (!start.current) return;
        onChange(Math.min(max, Math.max(min, start.current.w + e.clientX - start.current.x)));
      }}
      onPointerUp={(e) => {
        start.current = null;
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onDoubleClick={onReset}
    />
  );
}
