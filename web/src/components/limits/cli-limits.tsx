import { useEffect, useLayoutEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { chatText as t } from "@/text/chat";
import limitStyles from "./cli-limits.module.css";

const LimitsSchema = z.object({
  updated_at: z.number().nullable(),
  windows: z.array(z.object({ label: z.string(), used_percent: z.number().min(0).max(100).nullable(), status: z.string(), resets_at: z.number().nullable() })),
});

export function CliLimits({ providerId, name, kind }: { providerId: string; name: string; kind: string }) {
  const popup = useRef<HTMLDetailsElement>(null);
  const tooltip = useRef<HTMLSpanElement>(null);
  const panel = useRef<HTMLDivElement>(null);
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
  useLayoutEffect(() => {
    const anchor = popup.current;
    const summary = anchor?.querySelector("summary");
    const boxes = [tooltip.current, panel.current].filter((box): box is HTMLElement => box != null);
    if (!anchor || !summary || !boxes.length) return;

    let frame: number | undefined;
    const offsets = new Map<HTMLElement, number>();
    const updatePosition = () => {
      const viewport = window.visualViewport;
      let left = viewport?.offsetLeft ?? 0;
      let right = Math.min(left + (viewport?.width ?? window.innerWidth), document.documentElement.clientWidth);
      // The chat column clips its content, so a popup must stay inside it instead of sliding under the sidebar.
      const column = anchor.closest("main")?.getBoundingClientRect();
      if (column) { left = Math.max(left, column.left + 8); right = Math.min(right, column.right - 8); }
      const availableWidth = `${Math.max(0, right - left)}px`;
      if (anchor.style.getPropertyValue("--popup-max-width") !== availableWidth) {
        anchor.style.setProperty("--popup-max-width", availableWidth);
      }
      const anchorRight = anchor.getBoundingClientRect().right;
      let visible = false;
      for (const box of boxes) {
        const width = box === panel.current && !anchor.open ? 0 : box.getBoundingClientRect().width;
        if (!width) {
          delete box.dataset.positioned;
          continue;
        }
        visible = true;
        const preferredLeft = anchorRight - width;
        const fittedLeft = Math.max(left, Math.min(preferredLeft, right - width));
        const nextOffset = fittedLeft - preferredLeft;
        if (nextOffset !== (offsets.get(box) ?? 0)) {
          box.style.setProperty("--popup-right", `${-nextOffset}px`);
          offsets.set(box, nextOffset);
        }
        if (!box.dataset.positioned) box.dataset.positioned = "true";
      }
      // Wrapping can move the anchor without changing the size of either element.
      if (visible && frame === undefined) {
        frame = requestAnimationFrame(() => {
          frame = undefined;
          updatePosition();
        });
      }
    };

    const toggleFromSummary = (event: MouseEvent) => {
      if (event.defaultPrevented) return;
      event.preventDefault();
      // Native toggle events can be coalesced when the summary is activated rapidly.
      anchor.open = !anchor.open;
      updatePosition();
    };

    const observer = new ResizeObserver(updatePosition);
    boxes.forEach(box => observer.observe(box));
    summary.addEventListener("click", toggleFromSummary);
    anchor.addEventListener("toggle", updatePosition);
    return () => {
      summary.removeEventListener("click", toggleFromSummary);
      anchor.removeEventListener("toggle", updatePosition);
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, []);
  const now = Date.now() / 1000;
  const fresh = !query.isError && query.data?.updated_at != null && now - query.data.updated_at < 900;
  const stale = query.data != null && (query.data.updated_at == null || now - query.data.updated_at >= 900);
  const windows = (query.data?.windows ?? []).filter(
    ({ label, used_percent }) => !(label === "extra_usage" && Math.round(used_percent ?? 0) < 1),
  );
  const current = windows.filter(w => w.resets_at == null || w.resets_at > now);
  const percent = current.reduce<number | null>((max, w) => w.used_percent == null ? max : Math.max(max ?? 0, w.used_percent), null);
  const rejected = current.some(w => w.status === "rejected");
  const warning = percent == null ? current.some(w => w.status === "allowed_warning") : percent >= 70;
  const state = rejected || (percent != null && percent >= 90) ? "limit"
    : warning ? "warning" : percent === 0 ? "unused" : percent != null ? "normal" : "unknown";
  const statusLabel = rejected ? t.limitReached : percent != null ? t.limitUsed(Math.round(percent)) : warning ? t.limitWarning : t.limitUnknown;
  const providerName = kind === "codex_cli" ? "Codex"
    : kind === "claude_cli" ? "Claude"
    : kind === "antigravity_cli" ? "Antigravity" : name;
  const limitTitle = `Hạn mức ${providerName}`;
  const summaryLabel = `${limitTitle}: ${statusLabel}`;
  const updatedTime = query.data?.updated_at == null ? null : new Date(query.data.updated_at * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
  return <details ref={popup} className={limitStyles.limits}>
    <summary aria-label={summaryLabel}>
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" className={limitStyles.limitRing} data-state={state} data-stale={stale ? "true" : undefined}>
        <circle cx="12" cy="12" r="9" className={limitStyles.limitTrack} />
        {(percent != null || rejected) && <circle cx="12" cy="12" r="9" pathLength="100" strokeDasharray={`${rejected ? 100 : percent} 100`} className={limitStyles.limitProgress} />}
        {state === "unknown" && <text x="12" y="16" textAnchor="middle">?</text>}
        {warning && percent == null && !rejected && <text x="12" y="16" textAnchor="middle">!</text>}
      </svg>
      <span ref={tooltip} className={limitStyles.tooltip} aria-hidden="true">
        <span className={limitStyles.tooltipTitle}>{limitTitle}</span>
        <span>{statusLabel}</span>
      </span>
    </summary>
    <div ref={panel} className={limitStyles.panel} role="region" aria-label={`${t.limits} · ${name}`}>
      <p className={limitStyles.title}>{name}</p>
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
      {windows.length > 0 && !fresh && !query.isError && <p className={limitStyles.note}>{t.limitStale}{t.limitUpdated(updatedTime)}</p>}
      {!windows.length && !query.isPending && !query.isError && <p className={limitStyles.note}>{t.limitHint}</p>}
    </div>
  </details>;
}
