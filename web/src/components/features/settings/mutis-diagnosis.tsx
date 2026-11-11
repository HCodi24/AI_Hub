import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Stethoscope } from "lucide-react";
import { mutisApi } from "@/apis/mutis.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { DiagnosisSchema, type Diagnosis } from "@/schema/mutis.schema";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
const show = (value: unknown) => JSON.stringify(value);
const field = (data: unknown, key: string) => typeof data === "object" && data !== null ? (data as Record<string, unknown>)[key] : undefined;
/** AI diagnosis of the MutiS event log, with one-click (confirmed) setting changes. The server validates every change. */
export function MutisDiagnosis() {
  const client = useQueryClient();
  const history = useQuery({ queryKey: ["mutis-diagnoses"], queryFn: ({ signal }) => mutisApi.diagnoses(signal) });
  const [current, setCurrent] = useState<Diagnosis | null>(null); const [busy, setBusy] = useState(false); const [log, setLog] = useState("");
  const [pending, setPending] = useState<{ path: string; value?: unknown } | null>(null);
  const shown = current || history.data?.[0] || null;
  async function diagnose() {
    setBusy(true); setLog("");
    try {
      await mutisApi.diagnose(new AbortController().signal, (event, data) => {
        if (event === "log") setLog(text => `${text}${String(field(data, "text") ?? show(data))}\n`);
        if (event === "error") { const message = String(field(data, "message") ?? field(data, "text") ?? t.mutis.diagnoseFailed); setLog(text => `${text}${message}\n`); notify(message); }
        if (event === "diagnosis") { const parsed = DiagnosisSchema.safeParse(data); if (parsed.success) { setCurrent(parsed.data); notify(parsed.data.summary || t.mutis.diagnosis); } }
      });
      await client.invalidateQueries({ queryKey: ["mutis-diagnoses"] });
    } catch (error) { notify(errorText(error, t.error)); } finally { setBusy(false); }
  }
  async function apply(change: { path: string; value?: unknown }) {
    setPending(null);
    try {
      const result = await mutisApi.apply({ path: change.path, value: change.value });
      await client.invalidateQueries({ queryKey: ["mutis"] });
      notify(t.mutis.applied(change.path, show(result.old), show(result.new)));
    } catch (error) { notify(errorText(error, t.error)); }
  }
  return <section aria-label={t.mutis.diagnosis}><h3>{t.mutis.diagnosis}</h3>
    <div className={styles.actions}><Button disabled={busy} onClick={() => void diagnose()}><Stethoscope size={16} aria-hidden="true" />{busy ? t.mutis.diagnosing : t.mutis.diagnose}</Button>
      {(history.data?.length ?? 0) > 1 && <label className={styles.inlineLabel}>{t.mutis.history}
        <select value={shown?.ts ?? ""} onChange={event => setCurrent(history.data?.find(item => String(item.ts) === event.target.value) || null)}>
          {history.data?.map(item => <option key={item.ts} value={item.ts}>{new Date(item.ts * 1000).toLocaleString("vi-VN")} · {item.health}</option>)}</select></label>}</div>
    {log && <pre className={styles.log} aria-live="polite">{log}</pre>}
    {!shown && !history.isPending && <p className={styles.note}>{t.mutis.noDiagnosis}</p>}
    {shown && <article className={styles.group}><header className={styles.cardHead}><span className={styles.badge} data-on="true">{shown.health}</span></header>
      <p>{shown.summary}</p>
      {shown.findings.map((finding, index) => { const change = finding.setting_change;
        return <div key={index} className={styles.finding} data-severity={finding.severity}>
          <strong>{finding.problem} <span className={styles.badge}>{finding.severity}</span></strong>
          <p>{t.mutis.evidence}: {finding.evidence || "—"}</p><p>{t.mutis.cause}: {finding.cause || "—"}</p><p>{t.mutis.suggestion}: {finding.suggestion || "—"}</p>
          {change && (pending?.path === change.path
            ? <div className={styles.confirm} role="group"><p>{t.mutis.confirmApply(change.path, show(change.value))}</p>
              <Button variant="primary" onClick={() => void apply(change)}>{t.mutis.apply}</Button><Button onClick={() => setPending(null)}>{t.cancel}</Button></div>
            : <Button size="sm" onClick={() => setPending(change)}>{t.mutis.apply}: {change.path} = {show(change.value)}</Button>)}
        </div>; })}
    </article>}
  </section>;
}
