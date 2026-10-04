import { useEffect, useRef } from "react";

/** Pixels moved before a press counts as a drag rather than a click. */
const DRAG_THRESHOLD = 4;

/**
 * Pan a scrollable element by dragging it (from anywhere, nodes included)
 * and scroll it sideways with the mouse wheel. A drag swallows the click
 * that ends it, so dragging from a node does not select it.
 */
export function useDragScroll<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let start: { x: number; y: number; left: number; top: number; id: number } | null = null;
    let dragged = false;

    const down = (e: PointerEvent) => {
      if (e.button !== 0) return;
      start = { x: e.clientX, y: e.clientY, left: el.scrollLeft, top: el.scrollTop, id: e.pointerId };
      dragged = false;
    };
    const move = (e: PointerEvent) => {
      if (!start || e.pointerId !== start.id) return;
      const dx = e.clientX - start.x;
      const dy = e.clientY - start.y;
      if (!dragged && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
      if (!dragged) {
        dragged = true;
        el.setPointerCapture(e.pointerId);
        el.classList.add("dragging");
      }
      el.scrollLeft = start.left - dx;
      el.scrollTop = start.top - dy;
    };
    const up = (e: PointerEvent) => {
      if (!start || e.pointerId !== start.id) return;
      start = null;
      el.classList.remove("dragging");
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
    };
    // Capture phase: runs before the node's own click handler.
    const click = (e: MouseEvent) => {
      if (dragged) {
        e.stopPropagation();
        e.preventDefault();
        dragged = false;
      }
    };
    const wheel = (e: WheelEvent) => {
      // Trackpads already scroll sideways; only map plain vertical wheels.
      if (e.ctrlKey || e.deltaX !== 0 || el.scrollWidth <= el.clientWidth) return;
      e.preventDefault();
      el.scrollLeft += e.deltaMode === WheelEvent.DOM_DELTA_LINE ? e.deltaY * 32 : e.deltaY;
    };

    el.addEventListener("pointerdown", down);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    el.addEventListener("click", click, true);
    el.addEventListener("wheel", wheel, { passive: false });
    return () => {
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      el.removeEventListener("click", click, true);
      el.removeEventListener("wheel", wheel);
    };
  }, []);

  return ref;
}
