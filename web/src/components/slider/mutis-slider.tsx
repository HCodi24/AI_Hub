import type { CSSProperties, InputHTMLAttributes } from "react";
import styles from "./mutis-slider.module.css";

type MutisSliderProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "value" | "min" | "max" | "step" | "onChange" | "className"> & {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: number) => void;
  variant?: "smooth" | "pill";
  showTooltip?: boolean;
  formatValue?: (value: number) => string;
  ticks?: string[];
  className?: string;
};

export function MutisSlider({
  value, min = 0, max = 100, step = 1, onChange, variant = "smooth",
  showTooltip = variant === "smooth", formatValue = String, ticks, className, style, ...rest
}: MutisSliderProps) {
  const pct = max === min ? 0 : (value - min) / (max - min) * 100;
  const thumbHalf = variant === "smooth" ? 10 : 7;
  const tooltipLeft = `calc(${pct}% + ${thumbHalf - pct * thumbHalf * 2 / 100}px)`;
  const inputStyle: CSSProperties & { "--val": string } = { ...style, "--val": `${pct}%` };

  return <div className={className ? `${styles.wrap} ${className}` : styles.wrap}>
    <div className={styles.trackBox}>
      {showTooltip && <div className={styles.tooltip} style={{ left: tooltipLeft }} aria-hidden="true">
        {formatValue(value)}
      </div>}
      <input {...rest} type="range" min={min} max={max} step={step} value={value}
        className={`${styles.range} ${styles[variant]}`} style={inputStyle}
        onChange={event => onChange(Number(event.target.value))} />
    </div>
    {ticks && ticks.length > 0 && <div className={styles.ticks}>
      {ticks.map((tick, index) => <span key={index}>{tick}</span>)}
    </div>}
  </div>;
}
