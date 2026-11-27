"use client";

import { useLayoutEffect, type RefObject } from "react";

type Spring = {
  value: number;
  velocity: number;
  target: number;
  stiffness: number;
  damping: number;
  precision: number;
};

type Edge = "left" | "top" | "right" | "bottom";
type Box = Record<Edge, number>;
type Feel = readonly [response: number, bounce: number];

const EDGES: readonly Edge[] = ["left", "top", "right", "bottom"];
const LEAD: Feel = [0.2, 0.32];
const TRAIL: Feel = [0.3, 0.12];
const EVEN: Feel = [0.26, 0.22];
const DRAG_LEAD: Feel = [0.1, 0.12];
const DRAG_TRAIL: Feel = [0.18, 0.04];

const SWELL = 1;
const DRAG_THRESHOLD = 4;
const TRAVEL_SPEED = 90;
const ANTICIPATION = 0.5;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const createSpring = (response: number, bounce: number, value: number, precision = 0.001): Spring => ({
  value,
  velocity: 0,
  target: value,
  stiffness: ((2 * Math.PI) / response) ** 2,
  damping: (4 * Math.PI * (1 - bounce)) / response,
  precision,
});

const retuneSpring = (spring: Spring, response: number, bounce: number) => {
  spring.stiffness = ((2 * Math.PI) / response) ** 2;
  spring.damping = (4 * Math.PI * (1 - bounce)) / response;
};

const stepSpring = (spring: Spring, dt: number, snap: boolean): boolean => {
  if (snap || (spring.value === spring.target && spring.velocity === 0)) {
    spring.value = spring.target;
    spring.velocity = 0;
    return false;
  }

  const steps = Math.max(1, Math.ceil(dt * 120));
  const step = dt / steps;
  for (let i = 0; i < steps; i += 1) {
    spring.velocity += (-spring.stiffness * (spring.value - spring.target) - spring.damping * spring.velocity) * step;
    spring.value += spring.velocity * step;
  }

  if (
    Math.abs(spring.value - spring.target) < spring.precision &&
    Math.abs(spring.velocity) < spring.precision * 10
  ) {
    spring.value = spring.target;
    spring.velocity = 0;
    return false;
  }

  return true;
};

function motionReduced(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function boxWithin(element: HTMLElement, frame: HTMLElement): Box {
  const elRect = element.getBoundingClientRect();
  const frameRect = frame.getBoundingClientRect();
  const left = elRect.left - frameRect.left - frame.clientLeft + frame.scrollLeft;
  const top = elRect.top - frameRect.top - frame.clientTop + frame.scrollTop;
  return { left, top, right: left + elRect.width, bottom: top + elRect.height };
}

export type LiquidIndicatorOptions = {
  selected: string;
  item: string;
  draggable?: boolean;
  deformation?: number;
};

/**
 * Port of Glassy UI's 4-edge spring droplet indicator.
 * Stretches along its direction of travel while preserving volume, lifts subtly,
 * and settles with a spring bounce on segmented controls when glass style is active.
 */
export function useLiquidIndicator(
  rootRef: RefObject<HTMLElement | null>,
  indicatorRef: RefObject<HTMLElement | null>,
  { selected, item, draggable = true, deformation = 1 }: LiquidIndicatorOptions,
) {
  useLayoutEffect(() => {
    const root = rootRef.current;
    const indicator = indicatorRef.current;
    if (!root || !indicator) return;

    const edges: Record<Edge, Spring> = {
      left: createSpring(...EVEN, 0, 0.05),
      top: createSpring(...EVEN, 0, 0.05),
      right: createSpring(...EVEN, 0, 0.05),
      bottom: createSpring(...EVEN, 0, 0.05),
    };
    const lift = createSpring(0.22, 0.2, 0);

    let items: HTMLElement[] = [];
    let centres: number[] = [];
    let room = 0;
    let placed = false;
    let held: Element | null = null;
    let drag: { pointer: number; startX: number; offset: number; active: boolean } | null = null;
    let frame = 0;
    let last = 0;

    const frameOf = () => (indicator.offsetParent instanceof HTMLElement ? indicator.offsetParent : root);

    const paint = () => {
      const width = edges.right.value - edges.left.value;
      const height = edges.bottom.value - edges.top.value;
      const restWidth = Math.max(edges.right.target - edges.left.target, 1);
      const restHeight = Math.max(edges.bottom.target - edges.top.target, 1);
      const thinY = clamp(1 / Math.sqrt(Math.max(width / restWidth, 1)), 0.82, 1);
      const thinX = clamp(1 / Math.sqrt(Math.max(height / restHeight, 1)), 0.82, 1);
      const raised = Math.max(lift.value, 0) * deformation;
      const swell = raised * Math.min(room, SWELL);
      const drawnWidth = restWidth + (width * thinX - restWidth) * deformation + swell * 2;
      const drawnHeight = restHeight + (height * thinY - restHeight) * deformation + swell * 2;
      const centreX = (edges.left.value + edges.right.value) / 2;
      const centreY = (edges.top.value + edges.bottom.value) / 2;

      indicator.style.translate = `${centreX - drawnWidth / 2}px ${centreY - drawnHeight / 2}px`;
      indicator.style.width = `${drawnWidth}px`;
      indicator.style.height = `${drawnHeight}px`;
      indicator.style.setProperty("--lg-lift", raised.toFixed(3));
    };

    const step = (dt: number) => {
      const reduced = motionReduced();
      const travelling = EDGES.some(edge => Math.abs(edges[edge].velocity) > TRAVEL_SPEED);
      const pressed = held?.matches(selected) ?? false;
      const woken = held !== null ? ANTICIPATION : 0;

      lift.target = reduced ? 0 : pressed || drag?.active || travelling ? 1 : woken;
      let moving = false;
      for (const edge of EDGES) {
        moving = stepSpring(edges[edge], dt, reduced) || moving;
      }
      moving = stepSpring(lift, dt, reduced) || moving;
      paint();
      return moving;
    };

    const tick = (now: number) => {
      const dt = last ? Math.min((now - last) / 1000, 1 / 20) : 1 / 60;
      last = now;
      frame = 0;
      if (step(dt)) {
        frame = window.requestAnimationFrame(tick);
        return;
      }
      last = 0;
    };

    const startLoop = () => {
      if (!frame) frame = window.requestAnimationFrame(tick);
    };

    const stopLoop = () => {
      window.cancelAnimationFrame(frame);
      frame = 0;
      last = 0;
    };

    const current = (): Box => ({
      left: edges.left.value,
      top: edges.top.value,
      right: edges.right.value,
      bottom: edges.bottom.value,
    });

    const lean = (from: Box, to: Box, lead: Feel, trail: Feel) => {
      const axes: [Edge, Edge, number][] = [
        ["left", "right", (to.left + to.right - from.left - from.right) / 2],
        ["top", "bottom", (to.top + to.bottom - from.top - from.bottom) / 2],
      ];
      for (const [start, end, distance] of axes) {
        if (Math.abs(distance) < 1) {
          retuneSpring(edges[start], ...EVEN);
          retuneSpring(edges[end], ...EVEN);
        } else {
          retuneSpring(edges[distance > 0 ? end : start], ...lead);
          retuneSpring(edges[distance > 0 ? start : end], ...trail);
        }
      }
    };

    const aim = (to: Box, jump: boolean) => {
      for (const edge of EDGES) {
        edges[edge].target = to[edge];
        if (jump) {
          edges[edge].value = to[edge];
          edges[edge].velocity = 0;
        }
      }
    };

    const sync = (snap: boolean) => {
      const target = root.querySelector<HTMLElement>(selected);
      const container = frameOf();
      items = Array.from(root.querySelectorAll<HTMLElement>(item));
      const boxes = items.map(el => boxWithin(el, container));
      centres = boxes.map(box => (box.left + box.right) / 2);

      if (drag?.active) return;
      if (!target) {
        indicator.setAttribute("data-empty", "");
        placed = false;
        return;
      }

      const targetIndex = items.indexOf(target);
      const box = targetIndex < 0 ? boxWithin(target, container) : boxes[targetIndex];
      if (box.right - box.left <= 0 || box.bottom - box.top <= 0) return;

      const jump = snap || !placed || motionReduced();
      indicator.removeAttribute("data-empty");
      room = Math.max(0, Math.min(box.top, container.clientHeight - box.bottom));

      if (!jump) lean(current(), box, LEAD, TRAIL);
      aim(box, jump);
      placed = true;
      if (jump) paint();
      startLoop();
    };

    const pointerLocalX = (clientX: number) => {
      const container = frameOf();
      const rect = container.getBoundingClientRect();
      return clientX - rect.left - container.clientLeft + container.scrollLeft;
    };

    const follow = (event: PointerEvent) => {
      if (!drag || centres.length === 0) return;
      const width = edges.right.target - edges.left.target;
      const centre = clamp(
        pointerLocalX(event.clientX) - drag.offset,
        Math.min(...centres),
        Math.max(...centres),
      );
      const from = current();
      const to = { ...from, left: centre - width / 2, right: centre + width / 2 };
      lean(from, to, DRAG_LEAD, DRAG_TRAIL);
      aim({ ...to, top: edges.top.target, bottom: edges.bottom.target }, motionReduced());
      startLoop();
    };

    const release = (commit: boolean) => {
      if (!drag) return;
      const { active, pointer } = drag;
      drag = null;
      held = null;
      root.removeAttribute("data-dragging");
      if (root.hasPointerCapture(pointer)) root.releasePointerCapture(pointer);

      if (active && commit) {
        const centre = (edges.left.target + edges.right.target) / 2;
        let nearest = -1;
        centres.forEach((value, idx) => {
          if (items[idx].matches(":disabled")) return;
          if (nearest < 0 || Math.abs(value - centre) < Math.abs(centres[nearest] - centre)) {
            nearest = idx;
          }
        });
        if (nearest >= 0) items[nearest].click();
      }
      sync(false);
    };

    const onPointerDown = (event: PointerEvent) => {
      if (document.documentElement.dataset.style !== "glass") return;
      if (!event.isPrimary || event.button !== 0 || !(event.target instanceof Element)) return;
      const pressed = event.target.closest(item);
      if (!(pressed instanceof HTMLElement) || !root.contains(pressed) || pressed.matches(":disabled")) return;

      held = pressed;
      startLoop();

      if (!draggable || !pressed.matches(selected)) return;
      const centre = (edges.left.target + edges.right.target) / 2;
      drag = {
        pointer: event.pointerId,
        startX: event.clientX,
        offset: pointerLocalX(event.clientX) - centre,
        active: false,
      };
    };

    const onPointerMove = (event: PointerEvent) => {
      if (!drag || event.pointerId !== drag.pointer) return;
      if (!drag.active) {
        if (Math.abs(event.clientX - drag.startX) < DRAG_THRESHOLD) return;
        drag.active = true;
        root.setAttribute("data-dragging", "");
        root.setPointerCapture(event.pointerId);
      }
      follow(event);
    };

    const onPointerUp = (event: PointerEvent) => {
      if (drag && event.pointerId === drag.pointer) {
        release(event.type === "pointerup");
        return;
      }
      if (!held) return;
      held = null;
      startLoop();
    };

    const onLostCapture = (event: PointerEvent) => {
      if (event.target === root && drag?.active && event.pointerId === drag.pointer) release(false);
    };

    const resizes = new ResizeObserver(() => sync(true));
    const observeSizes = () => {
      resizes.disconnect();
      resizes.observe(root);
      root.querySelectorAll<HTMLElement>(item).forEach(el => resizes.observe(el));
    };

    const mutations = new MutationObserver(records => {
      if (records.some(r => r.type === "childList")) observeSizes();
      sync(false);
    });

    root.setAttribute("data-liquid-ready", "true");
    sync(true);
    observeSizes();
    mutations.observe(root, {
      attributes: true,
      attributeFilter: ["aria-pressed", "aria-current", "disabled"],
      childList: true,
      subtree: true,
    });
    root.addEventListener("pointerdown", onPointerDown);
    root.addEventListener("pointermove", onPointerMove);
    root.addEventListener("lostpointercapture", onLostCapture);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);

    return () => {
      stopLoop();
      mutations.disconnect();
      resizes.disconnect();
      root.removeEventListener("pointerdown", onPointerDown);
      root.removeEventListener("pointermove", onPointerMove);
      root.removeEventListener("lostpointercapture", onLostCapture);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
      root.removeAttribute("data-liquid-ready");
      root.removeAttribute("data-dragging");
      indicator.style.removeProperty("translate");
      indicator.style.removeProperty("width");
      indicator.style.removeProperty("height");
      indicator.style.removeProperty("--lg-lift");
    };
  }, [deformation, draggable, indicatorRef, item, rootRef, selected]);
}
