import { useEffect, useState } from "react";
import { Sun, Moon, Monitor } from "lucide-react";
import { chatText as t } from "@/text/chat";
import styles from "./theme-control.module.css";
type Theme = "light" | "dark" | "system";
export function ThemeControl() {
  const [theme, setTheme] = useState<Theme>("dark");
  useEffect(() => {
    const current = document.documentElement.dataset.theme;
    if (current === "light" || current === "system" || current === "dark") setTheme(current);
  }, []);
  function change(value: Theme) {
    setTheme(value); document.documentElement.dataset.theme = value;
    try { localStorage.setItem("ai-hub-theme", value); } catch { /* Keep in-memory preference. */ }
  }
  return <div className={styles.control} role="group" aria-label={t.appearance}>
    {([ ["light", Sun], ["dark", Moon], ["system", Monitor] ] as const).map(([value, Icon]) =>
      <button key={value} title={t[value]} aria-label={t[value]} aria-pressed={theme === value} onClick={() => change(value)}>
        <Icon size={16} aria-hidden="true" /><span>{t[value]}</span>
      </button>)}
  </div>;
}
