import { useEffect, useState, type CSSProperties } from "react";
import { RotateCcw } from "lucide-react";
import { settingsText as t } from "@/text/settings";
import styles from "./transparency-control.module.css";

export const TRANSPARENCY_STORAGE_KEY = "ai-hub-glass-transparency";
export const DEFAULT_TRANSPARENCY = 50;

const PRESETS = [
  { value: 0, label: t.appearance.transparencyLow },
  { value: 50, label: t.appearance.transparencyBalanced },
  { value: 100, label: t.appearance.transparencyHigh },
] as const;

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_TRANSPARENCY;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function applyTransparency(percent: number) {
  const clamped = clampPercent(percent);
  document.documentElement.style.setProperty("--glass-t", (clamped / 100).toFixed(2));
}

function saveTransparency(percent: number) {
  const clamped = clampPercent(percent);
  if (clamped === DEFAULT_TRANSPARENCY) localStorage.removeItem(TRANSPARENCY_STORAGE_KEY);
  else localStorage.setItem(TRANSPARENCY_STORAGE_KEY, String(clamped));
}

export function TransparencyControl({ id }: { id: string }) {
  const [value, setValue] = useState(DEFAULT_TRANSPARENCY);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(TRANSPARENCY_STORAGE_KEY);
      if (raw !== null) {
        const parsed = clampPercent(Number(raw));
        setValue(parsed);
        applyTransparency(parsed);
        return;
      }
    } catch { /* Storage unavailable. */ }
    applyTransparency(DEFAULT_TRANSPARENCY);
  }, []);

  function update(next: number) {
    const clamped = clampPercent(next);
    setValue(clamped);
    applyTransparency(clamped);
    try { saveTransparency(clamped); } catch { /* Storage unavailable. */ }
  }

  const sliderStyle = { "--slider-pct": `${value}%` } as CSSProperties;

  return <div data-glass="transparency-control" className={styles.control}>
    <div className={styles.sliderWrap}>
      <input
        id={id}
        className={styles.slider}
        style={sliderStyle}
        type="range"
        min={0}
        max={100}
        step={1}
        value={value}
        aria-valuetext={`${value}%`}
        onChange={event => update(event.currentTarget.valueAsNumber)}
      />
      <div className={styles.ticks}>
        {PRESETS.map(preset => (
          <button
            key={preset.value}
            type="button"
            className={styles.tick}
            data-active={value === preset.value || undefined}
            onClick={() => update(preset.value)}
          >
            {preset.label}
          </button>
        ))}
      </div>
    </div>
    <output className={styles.value} htmlFor={id}>{value}%</output>
    <button
      type="button"
      className={styles.reset}
      disabled={value === DEFAULT_TRANSPARENCY}
      title={t.appearance.resetTransparency}
      aria-label={t.appearance.resetTransparency}
      onClick={() => update(DEFAULT_TRANSPARENCY)}
    >
      <RotateCcw size={14} aria-hidden="true" />
    </button>
  </div>;
}
