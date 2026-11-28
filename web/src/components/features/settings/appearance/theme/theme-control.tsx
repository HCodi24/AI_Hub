import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { companionReact } from "@/components/features/settings/pet/companion/companion-store";
import { HorizonThemeToggle } from "./horizon-theme-toggle";
import { applyStyle, isStyle, resolveSavedStyle } from "./style-control";

type Theme = "light" | "dark";

function resolveTheme(value: unknown): Theme | undefined {
  if (value === "light" || value === "dark") return value;
  if (value === "glass") return "light";
  if (value === "system") {
    return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }
  return undefined;
}

function apply(value: Theme) {
  document.documentElement.dataset.theme = value;
  // localStorage is only a cache so the first paint has no flash; the server file is the source of truth.
  try { localStorage.setItem("ai-hub-theme", value); } catch { /* Keep in-memory preference. */ }
}

export function ThemeControl({ compact = false }: { compact?: boolean } = {}) {
  const client = useQueryClient();
  const [theme, setTheme] = useState<Theme>("dark");
  useEffect(() => {
    const root = document.documentElement;
    const syncFromDom = () => {
      const current = root.dataset.theme;
      if (current === "glass" && !root.dataset.style) {
        root.dataset.style = "glass";
      }
      const initial = resolveTheme(current);
      if (initial) {
        setTheme(initial);
        if (current === "glass" || current === "system") apply(initial);
      }
    };
    syncFromDom();
    const observer = new MutationObserver(syncFromDom);
    observer.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);
  const { data } = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const saved = resolveTheme(data?.theme);
  const savedStyle = resolveSavedStyle(data);
  useEffect(() => { if (saved) { setTheme(saved); apply(saved); } }, [saved]);
  useEffect(() => {
    if (!savedStyle) return;
    const requested = new URLSearchParams(window.location.search).get("style");
    if (isStyle(requested)) return;
    applyStyle(savedStyle);
  }, [savedStyle]);
  function change(value: Theme) {
    setTheme(value); apply(value);
    companionReact("themeChanged");
    configApi.saveTheme(value).then(() => client.invalidateQueries({ queryKey: ["config"] })).catch(() => undefined);
  }
  return (
    <HorizonThemeToggle
      isDark={theme === "dark"}
      size={compact ? "sm" : "md"}
      onToggle={() => change(theme === "dark" ? "light" : "dark")}
    />
  );
}
