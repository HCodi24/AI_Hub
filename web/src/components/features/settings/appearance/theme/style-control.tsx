import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sparkles, Square } from "lucide-react";
import { configApi } from "@/apis/config.api";
import { chatText as t } from "@/text/chat";
import styles from "./theme-control.module.css";

type Style = "default" | "glass";
const isStyle = (value: unknown): value is Style => value === "default" || value === "glass";
const STYLE_STORAGE_KEY = "ai-hub-style";
const STYLE_OPTIONS = [
  ["default", Square, t.styleDefault],
  ["glass", Sparkles, t.glass],
] as const;

function apply(value: Style) {
  document.documentElement.dataset.style = value;
  try { localStorage.setItem(STYLE_STORAGE_KEY, value); } catch { /* Giữ lựa chọn trong bộ nhớ. */ }
}

export function StyleControl() {
  const client = useQueryClient();
  const [style, setStyle] = useState<Style>("default");
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
      apply(initial);
    }
  }, []);
  const { data } = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const saved = data?.style === "" && data.theme === "glass" ? "glass" : data?.style;
  useEffect(() => {
    if (isStyle(saved)) {
      setStyle(saved);
      apply(saved);
    }
  }, [saved]);
  function change(value: Style) {
    setStyle(value);
    apply(value);
    configApi.saveStyle(value).then(() => client.invalidateQueries({ queryKey: ["config"] })).catch(() => undefined);
  }
  return <div data-glass="theme-control" className={styles.control} role="group" aria-label={t.style}>
    {STYLE_OPTIONS.map(([value, Icon, label]) =>
      <button key={value} type="button" title={label} aria-pressed={style === value} aria-label={label} onClick={() => change(value)}>
        <Icon size={16} aria-hidden="true" /><span>{label}</span>
      </button>)}
  </div>;
}
