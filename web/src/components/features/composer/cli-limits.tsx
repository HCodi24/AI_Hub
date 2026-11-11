import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { chatText as t } from "@/text/chat";
import styles from "./chat-toolbar.module.css";
import limitStyles from "./cli-limits.module.css";

const LimitsSchema = z.object({
  updated_at: z.number().nullable(),
  windows: z.array(z.object({ label: z.string(), used_percent: z.number().min(0).max(100).nullable(), status: z.string(), resets_at: z.number().nullable() })),
});

export function CliLimits({ providerId, name }: { providerId: string; name: string }) {
  const popup = useRef<HTMLDetailsElement>(null);
  const query = useQuery({
    queryKey: ["cli-limits", providerId],
    queryFn: async ({ signal }) => parseResponse(LimitsSchema, await http.get(`/providers/${encodeURIComponent(providerId)}/limits`, signal)),
    refetchInterval: 15000,
    retry: false,
  });
  useEffect(() => {
    function close(event: PointerEvent | KeyboardEvent) {
      if (!popup.current?.open) return;
      if (event instanceof KeyboardEvent) {
        if (event.key !== "Escape") return;
        event.preventDefault();
        popup.current.querySelector("summary")?.focus();
      } else if (event.target instanceof Node && popup.current.contains(event.target)) return;
      popup.current.open = false;
    }
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", close);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", close);
    };
  }, []);
  const now = Date.now() / 1000;
  const fresh = !query.isError && query.data?.updated_at != null && now - query.data.updated_at < 900;
  const windows = query.data?.windows || [];
  const current = fresh ? windows.filter(w => w.resets_at == null || w.resets_at > now) : [];
  const percent = current.reduce<number | null>((max, w) => w.used_percent == null ? max : Math.max(max ?? 0, w.used_percent), null);
  const rejected = current.some(w => w.status === "rejected");
  const warning = current.some(w => w.status === "allowed_warning") || (percent ?? 0) >= 80;
  const state = rejected ? "limit" : warning ? "warning" : percent != null ? "normal" : "unknown";
  const label = rejected ? t.limitReached : percent != null ? t.limitUsed(Math.round(percent)) : warning ? t.limitWarning : t.limitUnknown;
  return <details ref={popup} className={styles.limits}>
    <summary aria-label={`${t.limits}: ${label}`} title={`${t.limits}: ${label}`}>
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" className={styles.limitRing} data-state={state}>
        <circle cx="12" cy="12" r="9" className={styles.limitTrack} />
        {(percent != null || rejected) && <circle cx="12" cy="12" r="9" pathLength="100" strokeDasharray={`${rejected ? 100 : percent} 100`} className={styles.limitProgress} />}
        {state === "unknown" && <text x="12" y="16" textAnchor="middle">?</text>}
        {warning && percent == null && !rejected && <text x="12" y="16" textAnchor="middle">!</text>}
      </svg>
    </summary>
    <div className={`${styles.panel} ${limitStyles.panel}`} role="region" aria-label={`${t.limits} · ${name}`}>
      {query.isPending && <p className={limitStyles.note}>{t.loading}</p>}
      {query.isError && <p className={limitStyles.note}>{t.limitError}</p>}
      {windows.map((window, index) => <div key={`${window.label}-${index}`} className={limitStyles.window}>
        <div className={limitStyles.heading}>
          <span className={limitStyles.label}>{t.limitWindowLabels[window.label as keyof typeof t.limitWindowLabels] || window.label}</span>
          {window.resets_at != null && <time className={limitStyles.reset} dateTime={new Date(window.resets_at * 1000).toISOString()} title={new Date(window.resets_at * 1000).toLocaleString()}>
            {t.limitReset} {new Date(window.resets_at * 1000).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" })}
          </time>}
          <span className={limitStyles.percent}>{window.used_percent == null ? window.status === "rejected" ? t.limitReached : window.status === "allowed_warning" ? t.limitWarning : "—" : `${Math.round(window.used_percent)}%`}</span>
        </div>
        <div className={limitStyles.track} role="progressbar" aria-label={window.label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={window.used_percent ?? undefined}
          aria-valuetext={window.used_percent == null ? t.limitUnknown : t.limitUsed(Math.round(window.used_percent))}>
          {window.used_percent != null && <span style={{ width: `${window.used_percent}%` }} />}
        </div>
      </div>)}
      {windows.length > 0 && !fresh && !query.isError && <p className={limitStyles.note}>{t.limitStale}</p>}
      {!windows.length && !query.isPending && !query.isError && <p className={limitStyles.note}>{t.limitHint}</p>}
    </div>
  </details>;
}
