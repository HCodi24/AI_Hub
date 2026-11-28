import { useEffect, useState } from "react";
import { RotateCcw } from "lucide-react";
import { MutisSlider } from "@/components/slider/mutis-slider";
import { settingsText as t } from "@/text/settings";
import styles from "./transparency-control.module.css";

export const THICKNESS_STORAGE_KEY = "ai-hub-glass-thickness";
export const DEFAULT_THICKNESS = 1;
const MIN_THICKNESS = 0.25;
const MAX_THICKNESS = 2;

const PRESETS = [
  { value: 0.5, label: t.appearance.thicknessThin },
  { value: 1, label: t.appearance.thicknessRegular },
  { value: 1.75, label: t.appearance.thicknessThick },
] as const;

function clampThickness(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_THICKNESS;
  const clamped = Math.min(MAX_THICKNESS, Math.max(MIN_THICKNESS, value));
  return Math.round(clamped * 20) / 20;
}

function applyThickness(thickness: number) {
  const clamped = clampThickness(thickness);
  document.documentElement.style.setProperty("--glass-thickness", clamped.toFixed(2));
}

function saveThickness(thickness: number) {
  const clamped = clampThickness(thickness);
  if (clamped === DEFAULT_THICKNESS) localStorage.removeItem(THICKNESS_STORAGE_KEY);
  else localStorage.setItem(THICKNESS_STORAGE_KEY, clamped.toFixed(2));
}

export function ThicknessControl({ id }: { id: string }) {
  const [value, setValue] = useState(DEFAULT_THICKNESS);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(THICKNESS_STORAGE_KEY);
      if (raw !== null) {
        const parsed = clampThickness(Number(raw));
        setValue(parsed);
        applyThickness(parsed);
        return;
      }
    } catch { /* Storage unavailable. */ }
    applyThickness(DEFAULT_THICKNESS);
  }, []);

  function update(next: number) {
    const clamped = clampThickness(next);
    setValue(clamped);
    applyThickness(clamped);
    try { saveThickness(clamped); } catch { /* Storage unavailable. */ }
  }

  return <div data-glass="thickness-control" className={styles.control}>
    <div className={styles.sliderWrap}>
      <MutisSlider
        id={id}
        variant="smooth"
        showTooltip={false}
        min={MIN_THICKNESS}
        max={MAX_THICKNESS}
        step={0.05}
        value={value}
        aria-valuetext={`${value.toFixed(2)}x`}
        onChange={v => update(v)}
      />
      <div className={styles.ticks}>
        {PRESETS.map(preset => (
          <button
            key={preset.value}
            type="button"
            className={styles.tick}
            data-active={Math.abs(value - preset.value) < 0.01 || undefined}
            onClick={() => update(preset.value)}
          >
            {preset.label}
          </button>
        ))}
      </div>
    </div>
    <output className={styles.value} htmlFor={id}>{value.toFixed(2)}×</output>
    <button
      type="button"
      className={styles.reset}
      disabled={Math.abs(value - DEFAULT_THICKNESS) < 0.01}
      title={t.appearance.resetThickness}
      aria-label={t.appearance.resetThickness}
      onClick={() => update(DEFAULT_THICKNESS)}
    >
      <RotateCcw size={14} aria-hidden="true" />
    </button>
  </div>;
}
