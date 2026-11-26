import { useEffect, useRef } from "react";
import styles from "./tooltip.module.css";

const DELAY = 300;
const GUTTER = 8;

export function TooltipLayer() {
  const tooltipRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLSpanElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const tooltip = tooltipRef.current;
    const heading = headingRef.current;
    const body = textRef.current;
    if (!tooltip || !heading || !body || typeof tooltip.showPopover !== "function") return;

    let active: { element: Element; text: string; addedLabel: boolean } | undefined;
    let timer: number | undefined;
    const observer = new MutationObserver(() => {
      if (active && !active.element.isConnected) hide();
    });

    const clearTimer = () => {
      window.clearTimeout(timer);
      timer = undefined;
    };

    const hide = () => {
      clearTimer();
      observer.disconnect();
      if (active) {
        const { element, text, addedLabel } = active;
        if (!element.hasAttribute("title")) element.setAttribute("title", text);
        if (addedLabel && element.getAttribute("aria-label") === text) element.removeAttribute("aria-label");
        element.removeAttribute("data-tooltip-text");
        active = undefined;
      }
      delete tooltip.dataset.positioned;
      if (tooltip.matches(":popover-open")) tooltip.hidePopover();
    };

    const show = () => {
      clearTimer();
      if (!active?.element.isConnected) {
        hide();
        return;
      }
      const { element, text } = active;
      const title = element.getAttribute("data-tooltip-title")?.trim();
      heading.hidden = !title;
      heading.textContent = title ?? "";
      body.textContent = text;
      try {
        if (!tooltip.matches(":popover-open")) tooltip.showPopover();
      } catch {
        hide();
        return;
      }
      if (!tooltip.matches(":popover-open")) {
        hide();
        return;
      }

      const viewportWidth = document.documentElement.clientWidth;
      tooltip.style.setProperty("--tooltip-available-width", `${Math.max(0, viewportWidth - 2 * GUTTER)}px`);
      const rect = element.getBoundingClientRect();
      const { width, height } = tooltip.getBoundingClientRect();
      const left = Math.max(GUTTER, Math.min(rect.right - width, viewportWidth - width - GUTTER));
      const top = rect.top >= height + 2 * GUTTER ? rect.top - height - GUTTER : rect.bottom + GUTTER;
      tooltip.style.left = `${left}px`;
      tooltip.style.top = `${top}px`;
      tooltip.dataset.positioned = "true";
    };

    const findAnchor = (target: EventTarget | null) => {
      if (!(target instanceof Element) || target.matches("iframe")) return null;
      const titled = target.closest("[title]");
      // A stashed title still takes precedence over titles on its ancestors.
      if (active?.element.contains(target) && (!titled || !active.element.contains(titled))) return active.element;
      return titled;
    };

    const start = (element: Element, immediate: boolean) => {
      if (active?.element === element) {
        if (immediate && timer !== undefined) show();
        return;
      }
      const text = element.getAttribute("title");
      if (!text?.trim() || element.matches("iframe")) return;

      hide();
      const addedLabel = !element.hasAttribute("aria-label");
      active = { element, text, addedLabel };
      element.setAttribute("data-tooltip-text", text);
      element.removeAttribute("title");
      if (addedLabel) element.setAttribute("aria-label", text);
      observer.observe(document.body, { childList: true, subtree: true });
      if (immediate) show();
      else timer = window.setTimeout(show, DELAY);
    };

    const onPointerOver = (event: PointerEvent) => {
      if (event.pointerType !== "mouse" && event.pointerType !== "pen") return;
      const element = findAnchor(event.target);
      if (element) start(element, false);
    };
    const onPointerOut = (event: PointerEvent) => {
      if (event.pointerType !== "mouse" && event.pointerType !== "pen") return;
      const element = active?.element;
      if (element && event.target instanceof Node && element.contains(event.target)
        && !(event.relatedTarget instanceof Node && element.contains(event.relatedTarget))) hide();
    };
    const onFocusIn = (event: FocusEvent) => {
      const element = findAnchor(event.target);
      if (element?.matches(":focus-visible")) start(element, true);
    };
    const onFocusOut = (event: FocusEvent) => {
      if (event.target === active?.element) hide();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };

    document.addEventListener("pointerover", onPointerOver, true);
    document.addEventListener("pointerout", onPointerOut, true);
    document.addEventListener("focusin", onFocusIn, true);
    document.addEventListener("focusout", onFocusOut, true);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("scroll", hide, true);
    window.addEventListener("resize", hide);
    return () => {
      document.removeEventListener("pointerover", onPointerOver, true);
      document.removeEventListener("pointerout", onPointerOut, true);
      document.removeEventListener("focusin", onFocusIn, true);
      document.removeEventListener("focusout", onFocusOut, true);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("scroll", hide, true);
      window.removeEventListener("resize", hide);
      hide();
    };
  }, []);

  return (
    <div ref={tooltipRef} data-glass="tooltip" popover="manual" aria-hidden="true" className={styles.tooltip}>
      <span ref={headingRef} className={styles.title} hidden />
      <span ref={textRef} />
    </div>
  );
}
