import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { mutisApi } from "@/apis/mutis.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { Mutis } from "@/schema/mutis.schema";
import type { Provider } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { MutisDiagnosis } from "./mutis-diagnosis";
import { MutisStats } from "./mutis-stats";
import { errorText, useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
const ROLES = ["lead", "big", "small"] as const;
const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"] as const;
type BoolKey = "auto_plan" | "verify_gate" | "auto_review" | "auto_fix" | "retry_api" | "codex_watchdog" | "codex_auto_resume" | "deepseek_agent" | "event_log";
type NumKey = "fix_rounds_max" | "retry_attempts" | "retry_base_ms" | "watchdog_minutes" | "resume_max" | "deepseek_max_file_lines" | "deepseek_max_steps" | "debate_rounds";
/** One switch with the numeric limits that only matter while it is on. */
const SWITCHES: { key: BoolKey; numbers?: { key: NumKey; min: number; max: number }[] }[] = [
  { key: "auto_plan" }, { key: "verify_gate" }, { key: "auto_review" },
  { key: "auto_fix", numbers: [{ key: "fix_rounds_max", min: 0, max: 5 }] },
  { key: "retry_api", numbers: [{ key: "retry_attempts", min: 1, max: 5 }, { key: "retry_base_ms", min: 100, max: 60000 }] },
  { key: "codex_watchdog", numbers: [{ key: "watchdog_minutes", min: 1, max: 60 }] },
  { key: "codex_auto_resume", numbers: [{ key: "resume_max", min: 0, max: 3 }] },
  { key: "deepseek_agent", numbers: [{ key: "deepseek_max_file_lines", min: 50, max: 5000 }, { key: "deepseek_max_steps", min: 5, max: 40 }] },
  { key: "event_log" },
];
const modelsOf = (providers: Provider[], id: string) => providers.find(p => p.id === id)?.models || [];
export function MutisTab() {
  const client = useQueryClient();
  const { query: config } = useSettingsConfig();
  const query = useQuery({ queryKey: ["mutis"], queryFn: ({ signal }) => mutisApi.get(signal) });
  const [draft, setDraft] = useState<Mutis | null>(null); const [busy, setBusy] = useState(false);
  useEffect(() => { if (query.data) setDraft(query.data); }, [query.data]);
  if (query.isPending || !draft) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const providers = config.data?.providers || [];
  const edit = (patch: Partial<Mutis>) => setDraft(current => current && ({ ...current, ...patch }));
  const off = !draft.enabled;
  async function save() {
    if (!draft) return;
    setBusy(true);
    try { client.setQueryData(["mutis"], await mutisApi.put(draft)); notify(t.saved); }
    catch (error) { notify(errorText(error, t.error)); } finally { setBusy(false); }
  }
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void save(); }}>
    <label className={styles.check}><input type="checkbox" checked={draft.enabled} onChange={event => edit({ enabled: event.target.checked })} />
      <span><strong>{t.mutis.enabled}</strong></span></label>
    <fieldset className={styles.group} disabled={off}><legend>{t.mutis.roles}</legend>
      {ROLES.map(role => { const value = draft.roles[role];
        const setRole = (patch: Partial<typeof value>) => edit({ roles: { ...draft.roles, [role]: { ...value, ...patch } } });
        return <div key={role} className={styles.row}><strong>{t.mutis.role[role]}</strong>
          <label>{t.mutis.provider}<select value={value.provider_id} onChange={event => setRole({ provider_id: event.target.value, model: modelsOf(providers, event.target.value)[0] || "" })}>
            <option value="" />{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
          <label>{t.mutis.model}<input list={`mutis-models-${role}`} value={value.model} onChange={event => setRole({ model: event.target.value })} />
            <datalist id={`mutis-models-${role}`}>{modelsOf(providers, value.provider_id).map(m => <option key={m} value={m} />)}</datalist></label>
          <label>{t.mutis.effort}<select value={value.effort} onChange={event => setRole({ effort: event.target.value as typeof value.effort })}>
            {EFFORTS.map(effort => <option key={effort} value={effort}>{t.efforts[effort]}</option>)}</select></label></div>; })}
    </fieldset>
    <fieldset className={styles.group} disabled={off}><legend>{t.mutis.routing}</legend>
      <div className={styles.grid}>{draft.routing.map((rule, index) => <label key={rule.kind}>{t.mutis.kinds[rule.kind] || rule.kind}
        <select value={rule.role} onChange={event => edit({ routing: draft.routing.map((item, i) => i === index ? { ...item, role: event.target.value } : item) })}>
          {ROLES.map(role => <option key={role} value={role}>{t.mutis.role[role]}</option>)}</select></label>)}</div>
    </fieldset>
    <fieldset className={styles.group} disabled={off}><legend>{t.mutis.switches}</legend>
      {SWITCHES.map(item => <div key={item.key} className={styles.switch}>
        <label className={styles.check}><input type="checkbox" checked={draft[item.key]} onChange={event => edit({ [item.key]: event.target.checked })} /><span>{t.mutis[item.key]}</span></label>
        {item.numbers?.map(number => <label key={number.key}>{t.mutis[number.key]}<input type="number" min={number.min} max={number.max}
          value={draft[number.key]} disabled={!draft[item.key]} onChange={event => edit({ [number.key]: Number(event.target.value) })} /></label>)}
      </div>)}
      <label>{t.mutis.verify_command}<input value={draft.verify_command} placeholder={t.mutis.verify_commandHint} spellCheck={false}
        onChange={event => edit({ verify_command: event.target.value })} /></label>
      <label>{t.mutis.verify_ignore}<textarea rows={2} spellCheck={false} value={draft.verify_ignore.join("\n")}
        onChange={event => edit({ verify_ignore: event.target.value.split(/\r?\n/).map(x => x.trim()).filter(Boolean) })} /></label>
      <div className={styles.grid}>
        <label>{t.mutis.reviewer}<select value={draft.reviewer_provider_id} onChange={event => edit({ reviewer_provider_id: event.target.value, reviewer_model: modelsOf(providers, event.target.value)[0] || "" })}>
          <option value="" />{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
        <label>{t.mutis.reviewerModel}<input list="mutis-reviewer-models" value={draft.reviewer_model} onChange={event => edit({ reviewer_model: event.target.value })} />
          <datalist id="mutis-reviewer-models">{modelsOf(providers, draft.reviewer_provider_id).map(m => <option key={m} value={m} />)}</datalist></label>
      </div>
    </fieldset>
    <fieldset className={styles.group}><legend>{t.mutis.debate}</legend><div className={styles.grid}>
      <label>{t.mutis.debate_rounds}<input type="number" min={1} max={3} value={draft.debate_rounds} onChange={event => edit({ debate_rounds: Number(event.target.value) })} /></label>
      <label>{t.mutis.debate_judge_model}<input value={draft.debate_judge_model} placeholder={t.mutis.debateJudgeHint} onChange={event => edit({ debate_judge_model: event.target.value })} /></label>
    </div></fieldset>
    <label>{t.mutis.worker_rules}<textarea rows={5} value={draft.worker_rules} onChange={event => edit({ worker_rules: event.target.value })} /></label>
    <div className={styles.actions}><Button type="submit" variant="primary" disabled={busy}>{t.save}</Button></div>
    <MutisDiagnosis />
    <MutisStats />
  </form>;
}
