import { useLayoutEffect, type RefObject } from "react";

export function usePopupFit(
  anchorRef: RefObject<HTMLDetailsElement | null>,
  panelRef: RefObject<HTMLElement | null>,
  onOpenChange?: (open: boolean) => void,
) {
  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const summary = anchor?.querySelector("summary");
    const box = panelRef.current;
    if (!anchor || !box) return;

    let frame: number | undefined;
    let offset = -(Number.parseFloat(box.style.getPropertyValue("--popup-right")) || 0);
    const updatePosition = () => {
      const viewport = window.visualViewport;
      let left = viewport?.offsetLeft ?? 0;
      let right = Math.min(left + (viewport?.width ?? window.innerWidth), document.documentElement.clientWidth);
      // The chat column clips its content, so a popup must stay inside it instead of sliding under the sidebar.
      const column = anchor.closest("main")?.getBoundingClientRect();
      if (column) { left = Math.max(left, column.left + 8); right = Math.min(right, column.right - 8); }
      const availableWidth = `${Math.max(0, right - left)}px`;
      if (anchor.style.getPropertyValue("--popup-max-width") !== availableWidth) {
        anchor.style.setProperty("--popup-max-width", availableWidth);
      }
      const width = anchor.open ? box.getBoundingClientRect().width : 0;
      if (!width) {
        delete box.dataset.positioned;
        if (frame !== undefined) {
          cancelAnimationFrame(frame);
          frame = undefined;
        }
        return;
      }
      const anchorRight = anchor.getBoundingClientRect().right;
      const preferredLeft = anchorRight - width;
      const fittedLeft = Math.max(left, Math.min(preferredLeft, right - width));
      const nextOffset = fittedLeft - preferredLeft;
      if (nextOffset !== offset) {
        box.style.setProperty("--popup-right", `${-nextOffset}px`);
        offset = nextOffset;
      }
      if (!box.dataset.positioned) box.dataset.positioned = "true";
      // Wrapping can move the anchor without changing the size of either element.
      if (frame === undefined) {
        frame = requestAnimationFrame(() => {
          frame = undefined;
          updatePosition();
        });
      }
    };

    const toggleFromSummary = (event: MouseEvent) => {
      if (event.defaultPrevented) return;
      event.preventDefault();
      // Native toggle events can be coalesced when the summary is activated rapidly.
      anchor.open = !anchor.open;
      onOpenChange?.(anchor.open);
      updatePosition();
    };

    const observer = new ResizeObserver(updatePosition);
    observer.observe(box);
    if (onOpenChange) summary?.addEventListener("click", toggleFromSummary);
    anchor.addEventListener("toggle", updatePosition);
    updatePosition();
    return () => {
      summary?.removeEventListener("click", toggleFromSummary);
      anchor.removeEventListener("toggle", updatePosition);
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [anchorRef, panelRef, onOpenChange]);
}
