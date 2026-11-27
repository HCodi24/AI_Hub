import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sun, Moon, Monitor } from "lucide-react";
import { configApi } from "@/apis/config.api";
import { useLiquidIndicator } from "@/components/features/settings/appearance/theme/glass/use-liquid-indicator";
import { chatText as t } from "@/text/chat";
import styles from "./theme-control.module.css";
type Theme = "light" | "dark" | "system";
const isTheme = (value: unknown): value is Theme =>
  value === "light" || value === "dark" || value === "system";
const THEME_OPTIONS = [
  ["light", Sun],
  ["dark", Moon],
  ["system", Monitor],
] as const;

function apply(value: Theme) {
  document.documentElement.dataset.theme = value;
  // localStorage is only a cache so the first paint has no flash; the server file is the source of truth.
  try { localStorage.setItem("ai-hub-theme", value); } catch { /* Keep in-memory preference. */ }
}
export function ThemeControl() {
  const client = useQueryClient();
  const [theme, setTheme] = useState<Theme>("dark");
  const rootRef = useRef<HTMLDivElement>(null);
  const indicatorRef = useRef<HTMLSpanElement>(null);
  useLiquidIndicator(rootRef, indicatorRef, {
    selected: 'button[aria-pressed="true"]',
    item: "button",
    draggable: true,
  });
  useEffect(() => {
    const { dataset } = document.documentElement;
    const current = dataset.theme;
    if (current === "glass" && !dataset.style) {
      dataset.style = "glass";
    }
    const initial = current === "glass" ? "light" : current;
    if (isTheme(initial)) {
      setTheme(initial);
      if (current === "glass") apply(initial);
    }
  }, []);
  const savedTheme = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data?.theme;
  const saved = savedTheme === "glass" ? "light" : savedTheme;
  useEffect(() => { if (isTheme(saved)) { setTheme(saved); apply(saved); } }, [saved]);
  function change(value: Theme) {
    setTheme(value); apply(value);
    configApi.saveTheme(value).then(() => client.invalidateQueries({ queryKey: ["config"] })).catch(() => undefined);
  }
  return <div ref={rootRef} data-glass="theme-control" className={styles.control} role="group" aria-label={t.appearance}>
    <span ref={indicatorRef} data-glass="liquid-indicator" aria-hidden="true" />
    {THEME_OPTIONS.map(([value, Icon]) =>
      <button key={value} type="button" title={t[value]} aria-pressed={theme === value} aria-label={t[value]} onClick={() => change(value)}>
        <Icon size={16} aria-hidden="true" /><span>{t[value]}</span>
      </button>)}
  </div>;
}
