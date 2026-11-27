"use client";

import { useEffect, useState } from "react";
import {
  buildLensMapDataUrl,
  clamp,
  MAX_LENS_SURFACE_AREA,
  type LensMapMetrics,
  type LensRadii,
} from "./refraction-map";

/**
 * Surfaces that receive the Chromium Snell's-law edge refraction lens when
 * data-style="glass" is active. Oversized surfaces (> 720,000 px^2) are
 * automatically skipped to keep scrolling smooth at 60 fps.
 */
const LENS_SELECTOR = [
  '[data-glass="main"]',
  '[data-glass="preview-panel"]',
  '[data-glass="composer-box"]',
  '[data-glass="model-group"]',
  '[data-glass="debate-button"]',
  '[data-glass="permission-picker"] > summary',
  'button[data-glass="terminal-toggle"]',
  'button[data-glass="compact-button"]',
  'button[data-glass="preview-toggle"]',
  '[data-glass="window-controls"]',
  '[data-glass="menu-labels"]',
  '[data-glass="theme-control"]',
  '[data-glass="skill-menu"]',
  '[data-glass="preferences-panel"]',
  '[data-glass="overlay"]',
  '[data-glass="tooltip"]',
  '[data-glass="card"]',
  'aside[data-glass="dashboard"]',
  'dialog[data-glass="dialog"]',
  '[data-glass="diff-panel"]',
  '[data-glass="rail"]',
  '[data-glass="sidebar-content"]',
].join(", ");

const LENS_BEVEL = 36;
const LENS_THICKNESS_RATIO = 1.4;
const MIN_FROST = 0.5;

type LensEntry = {
  id: string;
  blur: number;
  bodyBrightness: number;
  dispersion: number;
  href: string;
  metrics: LensMapMetrics;
  rimBrightness: number;
  saturation: number;
};

// Only Chromium renders SVG url() filters inside backdrop-filter, and only Chromium exposes userAgentData.
function supportsBackdropLens(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof navigator !== "undefined" &&
    "userAgentData" in navigator &&
    typeof ResizeObserver !== "undefined"
  );
}

function readNumber(style: CSSStyleDeclaration, name: string, fallback = 0): number {
  const value = Number.parseFloat(style.getPropertyValue(name));
  return Number.isFinite(value) ? value : fallback;
}

function readRadius(value: string, width: number, height: number): number {
  const [first] = value.trim().split(/\s+/);
  const largest = Math.min(width, height) / 2;
  const parsed = Number.parseFloat(first);
  if (first.endsWith("%")) return clamp((parsed / 100) * Math.min(width, height), 0, largest);
  return clamp(Number.isFinite(parsed) ? parsed : largest, 0, largest);
}

function measureElementLens(el: HTMLElement, id: string): LensEntry | null {
  const width = el.offsetWidth;
  const height = el.offsetHeight;
  if (!width || !height || width * height > MAX_LENS_SURFACE_AREA) return null;

  const style = window.getComputedStyle(el);
  if (style.display === "none" || style.visibility === "hidden") return null;

  const depth = readNumber(style, "--lg-lens-depth");
  if (depth <= 0) return null;

  const radii: LensRadii = [
    readRadius(style.borderTopLeftRadius, width, height),
    readRadius(style.borderTopRightRadius, width, height),
    readRadius(style.borderBottomRightRadius, width, height),
    readRadius(style.borderBottomLeftRadius, width, height),
  ];

  const multiplier = clamp(readNumber(style, "--glass-thickness", 1), 0.25, 2);
  const bevelToken = readNumber(style, "--lg-lens-bevel");
  const thicknessToken = readNumber(style, "--lg-lens-thickness");
  const baseBevel = bevelToken > 0 ? bevelToken : clamp(Math.max(...radii), 6, LENS_BEVEL);

  const built = buildLensMapDataUrl({
    bevel: clamp(baseBevel * (0.6 + 0.4 * multiplier), 4, Math.min(width, height) / 2),
    depth,
    devicePixelRatio: window.devicePixelRatio || 1,
    height,
    ior: Math.max(1, readNumber(style, "--lg-lens-ior", 1.5)),
    radii,
    thickness: (thicknessToken > 0 ? thicknessToken : baseBevel * LENS_THICKNESS_RATIO) * multiplier,
    width,
  });

  if (!built) return null;

  return {
    id,
    blur: Math.max(0, readNumber(style, "--lg-lens-blur", 4)),
    bodyBrightness: clamp(readNumber(style, "--lg-lens-body-brightness", 1), 0, 1),
    dispersion: clamp(readNumber(style, "--lg-lens-dispersion", 0.14) * multiplier, 0, 1),
    href: built.href,
    metrics: built.metrics,
    rimBrightness: clamp(readNumber(style, "--lg-lens-rim-brightness", 1), 0, 1),
    saturation: readNumber(style, "--lg-lens-saturation", 1.4) || 1,
  };
}

function sameLens(a: LensEntry | undefined, b: LensEntry): boolean {
  return (
    a !== undefined &&
    a.id === b.id &&
    a.href === b.href &&
    a.blur === b.blur &&
    a.bodyBrightness === b.bodyBrightness &&
    a.dispersion === b.dispersion &&
    a.rimBrightness === b.rimBrightness &&
    a.saturation === b.saturation
  );
}

function LensFilterDef({ lens }: { lens: LensEntry }) {
  const frost = Math.max(lens.blur, MIN_FROST);
  const padding = Math.ceil(frost * 3) + 2;
  const scale = Math.round(2 * lens.metrics.reach * 100) / 100;

  const bend = (input: string, factor: number, result: string) => (
    <feDisplacementMap
      in={input}
      in2="lens"
      scale={Math.round(scale * factor * 100) / 100}
      xChannelSelector="R"
      yChannelSelector="G"
      result={result}
    />
  );

  const channel = (name: string, matrix: string, factor: number) => (
    <>
      {bend("frost", factor, `${name}-scene`)}
      <feColorMatrix in={`${name}-scene`} type="matrix" values={matrix} result={`${name}-bent`} />
    </>
  );

  const dim = (brightness: number, result: string) =>
    brightness < 1 ? (
      <feComponentTransfer in="bent" result={result}>
        <feFuncR type="linear" slope={brightness} />
        <feFuncG type="linear" slope={brightness} />
        <feFuncB type="linear" slope={brightness} />
      </feComponentTransfer>
    ) : null;

  const shaded = lens.rimBrightness !== lens.bodyBrightness;
  const rimSource = lens.rimBrightness < 1 ? "rim-lit" : "bent";
  const topSource = lens.bodyBrightness < 1 ? "top-lit" : "bent";

  return (
    <filter
      id={lens.id}
      x={-padding}
      y={-padding}
      width={lens.metrics.width + padding * 2}
      height={lens.metrics.height + padding * 2}
      filterUnits="userSpaceOnUse"
      primitiveUnits="userSpaceOnUse"
      colorInterpolationFilters="sRGB"
    >
      <feGaussianBlur in="SourceGraphic" stdDeviation={frost} result="frost" />
      <feImage
        href={lens.href}
        x={0}
        y={0}
        width={lens.metrics.width}
        height={lens.metrics.height}
        preserveAspectRatio="none"
        result="lens"
      />
      {lens.dispersion > 0.005 ? (
        <>
          {channel("red", "1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1", 1 - lens.dispersion / 2)}
          {channel("green", "0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 1", 1)}
          {channel("blue", "0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1", 1 + lens.dispersion / 2)}
          <feBlend in="red-bent" in2="green-bent" mode="screen" result="red-green" />
          <feBlend in="red-green" in2="blue-bent" mode="screen" result="rgb" />
          <feComposite in="rgb" in2="green-scene" operator="in" result="bent" />
        </>
      ) : (
        bend("frost", 1, "bent")
      )}
      {shaded ? (
        <>
          <feColorMatrix
            in="lens"
            type="matrix"
            values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0"
            result="rim"
          />
          {dim(lens.rimBrightness, "rim-lit")}
          {dim(lens.bodyBrightness, "top-lit")}
          <feComposite in={rimSource} in2="rim" operator="in" result="rim-glass" />
          <feComposite in="rim-glass" in2={topSource} operator="over" result="glass" />
        </>
      ) : (
        dim(lens.bodyBrightness, "top-lit")
      )}
      {lens.saturation === 1 ? null : (
        <feColorMatrix
          in={shaded ? "glass" : topSource}
          type="saturate"
          values={String(lens.saturation)}
        />
      )}
    </filter>
  );
}

/**
 * Global Glassy UI refraction lens host for MutiS.
 * Measures every eligible [data-glass] element in glass mode and binds a
 * real Snell's-law rim displacement filter via --glass-lens-filter.
 */
export function GlassyLensEngine() {
  const [lenses, setLenses] = useState<LensEntry[]>([]);

  useEffect(() => {
    if (!supportsBackdropLens()) return;

    const root = document.documentElement;
    const transparencyQuery = window.matchMedia("(prefers-reduced-transparency: reduce)");
    const contrastQuery = window.matchMedia("(prefers-contrast: more)");

    const idMap = new WeakMap<HTMLElement, string>();
    const activeMap = new Map<HTMLElement, LensEntry>();
    const observed = new Set<HTMLElement>();
    let nextId = 1;
    let raf = 0;

    const clearElement = (el: HTMLElement) => {
      delete el.dataset.glassLens;
      el.style.removeProperty("--glass-lens-filter");
      activeMap.delete(el);
    };

    const clearAll = () => {
      activeMap.forEach((_, el) => {
        delete el.dataset.glassLens;
        el.style.removeProperty("--glass-lens-filter");
      });
      activeMap.clear();
      observed.forEach(el => resizeObserver.unobserve(el));
      observed.clear();
      setLenses(prev => (prev.length === 0 ? prev : []));
    };

    const getElementId = (el: HTMLElement) => {
      let id = idMap.get(el);
      if (!id) {
        id = `mutis-glass-lens-${nextId++}`;
        idMap.set(el, id);
      }
      return id;
    };

    const sync = () => {
      raf = 0;
      if (root.dataset.style !== "glass" || transparencyQuery.matches) {
        clearAll();
        return;
      }

      const matched = new Set(Array.from(document.querySelectorAll<HTMLElement>(LENS_SELECTOR)));
      let changed = false;

      // Remove stale elements no longer in the DOM or no longer matching.
      observed.forEach(el => {
        if (!matched.has(el) || !el.isConnected) {
          resizeObserver.unobserve(el);
          observed.delete(el);
          if (activeMap.has(el)) {
            clearElement(el);
            changed = true;
          }
        }
      });

      // Measure every matched element.
      matched.forEach(el => {
        if (!observed.has(el)) {
          resizeObserver.observe(el, { box: "border-box" });
          observed.add(el);
        }

        const id = getElementId(el);
        const measured = measureElementLens(el, id);

        if (!measured) {
          if (activeMap.has(el)) {
            clearElement(el);
            changed = true;
          }
          return;
        }

        const previous = activeMap.get(el);
        if (!sameLens(previous, measured)) {
          activeMap.set(el, measured);
          el.dataset.glassLens = "active";
          el.style.setProperty("--glass-lens-filter", `url(#${id})`);
          changed = true;
        }
      });

      if (changed) {
        setLenses(Array.from(activeMap.values()));
      }
    };

    const schedule = () => {
      if (!raf) raf = window.requestAnimationFrame(sync);
    };

    const resizeObserver = new ResizeObserver(schedule);
    const rootObserver = new MutationObserver(schedule);
    const bodyObserver = new MutationObserver(records => {
      // Ignore mutations inside our own hidden SVG defs container.
      for (const record of records) {
        const target = record.target;
        if (target instanceof Element && target.closest("[data-mutis-glass-defs]")) continue;
        schedule();
        return;
      }
    });

    rootObserver.observe(root, {
      attributes: true,
      attributeFilter: ["data-style", "data-theme", "style"],
    });
    bodyObserver.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["open", "aria-expanded", "data-glass"],
    });

    transparencyQuery.addEventListener("change", schedule);
    contrastQuery.addEventListener("change", schedule);
    window.addEventListener("resize", schedule, { passive: true });
    schedule();

    return () => {
      window.cancelAnimationFrame(raf);
      rootObserver.disconnect();
      bodyObserver.disconnect();
      resizeObserver.disconnect();
      transparencyQuery.removeEventListener("change", schedule);
      contrastQuery.removeEventListener("change", schedule);
      window.removeEventListener("resize", schedule);
      clearAll();
    };
  }, []);

  if (lenses.length === 0) return null;

  return (
    <svg
      aria-hidden="true"
      data-mutis-glass-defs=""
      focusable="false"
      viewBox="0 0 0 0"
      style={{
        position: "fixed",
        width: 0,
        height: 0,
        overflow: "hidden",
        pointerEvents: "none",
      }}
    >
      <defs>
        {lenses.map(lens => (
          <LensFilterDef key={lens.id} lens={lens} />
        ))}
      </defs>
    </svg>
  );
}
