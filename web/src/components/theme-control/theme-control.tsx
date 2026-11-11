import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sun, Moon, Monitor } from "lucide-react";
import { configApi } from "@/apis/config.api";
import { chatText as t } from "@/text/chat";
import styles from "./theme-control.module.css";
type Theme = "light" | "dark" | "system";
const isTheme = (value: unknown): value is Theme => value === "light" || value === "dark" || value === "system";
function apply(value: Theme) {
  document.documentElement.dataset.theme = value;
  // localStorage is only a cache so the first paint has no flash; the server file is the source of truth.
  try { localStorage.setItem("ai-hub-theme", value); } catch { /* Keep in-memory preference. */ }
}
export function ThemeControl() {
  const client = useQueryClient();
  const [theme, setTheme] = useState<Theme>("dark");
  useEffect(() => {
    const current = document.documentElement.dataset.theme;
    if (isTheme(current)) setTheme(current);
  }, []);
  const saved = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data?.theme;
  useEffect(() => { if (isTheme(saved)) { setTheme(saved); apply(saved); } }, [saved]);
  function change(value: Theme) {
    setTheme(value); apply(value);
    configApi.saveTheme(value).then(() => client.invalidateQueries({ queryKey: ["config"] })).catch(() => undefined);
  }
  return <div className={styles.control} role="group" aria-label={t.appearance}>
    {([ ["light", Sun], ["dark", Moon], ["system", Monitor] ] as const).map(([value, Icon]) =>
      <button key={value} title={t[value]} aria-pressed={theme === value} aria-label={t[value]} onClick={() => change(value)}>
        <Icon size={16} aria-hidden="true" /><span>{t[value]}</span>
      </button>)}
  </div>;
}
