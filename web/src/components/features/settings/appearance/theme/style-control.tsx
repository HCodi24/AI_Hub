import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AppWindow, Sparkles } from "lucide-react";
import { configApi } from "@/apis/config.api";
import { useLiquidIndicator } from "@/components/features/settings/appearance/theme/glass/use-liquid-indicator";
import { chatText as t } from "@/text/chat";
import styles from "./theme-control.module.css";

export type Style = "default" | "glass";
export const isStyle = (value: unknown): value is Style => value === "default" || value === "glass";
const STYLE_STORAGE_KEY = "ai-hub-style";
const STYLE_OPTIONS = [
  ["default", AppWindow, t.styleDefault],
  ["glass", Sparkles, t.glass],
] as const;

export function applyStyle(value: Style) {
  document.documentElement.dataset.style = value;
  try { localStorage.setItem(STYLE_STORAGE_KEY, value); } catch { /* Giữ lựa chọn trong bộ nhớ. */ }
}

export function resolveSavedStyle(data?: { style?: string; theme?: string }): Style | undefined {
  const saved = data?.style === "" && data.theme === "glass" ? "glass" : data?.style;
  return isStyle(saved) ? saved : undefined;
}

export function StyleControl() {
  const client = useQueryClient();
  const [style, setStyle] = useState<Style>("default");
  const rootRef = useRef<HTMLDivElement>(null);
  const indicatorRef = useRef<HTMLSpanElement>(null);
  useLiquidIndicator(rootRef, indicatorRef, {
    selected: 'button[aria-pressed="true"]',
    item: "button",
    draggable: true,
  });
  useEffect(() => {
    const { style: current, theme } = document.documentElement.dataset;
    let initial = isStyle(current) ? current : undefined;
    if (!initial) {
      try {
        const cached = localStorage.getItem(STYLE_STORAGE_KEY);
        if (isStyle(cached)) initial = cached;
      } catch { /* Tiếp tục khi bộ nhớ cục bộ bị chặn. */ }
    }
    if (!initial && theme === "glass") initial = "glass";
    if (isStyle(initial)) {
      setStyle(initial);
      applyStyle(initial);
    }
  }, []);
  const { data } = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const saved = resolveSavedStyle(data);
  useEffect(() => {
    if (saved) {
      setStyle(saved);
      applyStyle(saved);
    }
  }, [saved]);
  function change(value: Style) {
    setStyle(value);
    applyStyle(value);
    configApi.saveStyle(value).then(() => client.invalidateQueries({ queryKey: ["config"] })).catch(() => undefined);
  }
  return <div ref={rootRef} data-glass="theme-control" data-surface={style} className={styles.control} role="group" aria-label={t.style}>
    <span ref={indicatorRef} data-glass="liquid-indicator" className={styles.indicator} aria-hidden="true" />
    {STYLE_OPTIONS.map(([value, Icon, label]) =>
      <button
        key={value}
        type="button"
        data-style-option={value}
        title={label}
        aria-pressed={style === value}
        onClick={() => change(value)}
      >
        <Icon size={14} strokeWidth={1.9} aria-hidden="true" /><span>{label}</span>
      </button>)}
  </div>;
}
