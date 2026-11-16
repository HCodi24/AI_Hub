import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { mutisApi } from "@/apis/mutis.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { Switch } from "@/components/button/switch";
import { notify } from "@/components/toast/toast";
import { MutisSchema, type Mutis } from "@/schema/mutis.schema";
import type { Provider } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { MutisDiagnosis } from "./mutis-diagnosis";
import { MutisStats } from "./mutis-stats";
import { VerifyProject } from "./verify-project";
import { errorText, useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
import { useSettingsAutosave } from "./use-settings-autosave";
import { effortsFor, effortFor } from "@/constants/chat";
const ROLES = ["lead", "big", "small"] as const;
const PROFILES = ["high_roles", "roles"] as const;
type Profile = (typeof PROFILES)[number];
type BoolKey = "delegate_checker" | "deepseek_ui" | "verify_gate" | "auto_review" | "auto_fix" | "retry_api" | "codex_watchdog" | "codex_auto_resume" | "deepseek_agent" | "event_log";
type NumKey = "fix_rounds_max" | "retry_attempts" | "retry_base_ms" | "watchdog_minutes" | "resume_max" | "deepseek_max_file_lines" | "deepseek_max_steps" | "debate_rounds";
/** One switch with the numeric limits that only matter while it is on. */
const SWITCHES: { key: BoolKey; numbers?: { key: NumKey; min: number; max: number }[] }[] = [
  { key: "verify_gate" }, { key: "auto_review" },
  { key: "auto_fix", numbers: [{ key: "fix_rounds_max", min: 0, max: 5 }] },
  { key: "retry_api", numbers: [{ key: "retry_attempts", min: 1, max: 5 }, { key: "retry_base_ms", min: 100, max: 60000 }] },
  { key: "codex_watchdog", numbers: [{ key: "watchdog_minutes", min: 1, max: 60 }] },
  { key: "codex_auto_resume", numbers: [{ key: "resume_max", min: 0, max: 3 }] },
  { key: "deepseek_agent", numbers: [{ key: "deepseek_max_file_lines", min: 50, max: 5000 }, { key: "deepseek_max_steps", min: 5, max: 40 }] },
  { key: "event_log" },
];
const modelsOf = (providers: Provider[], id: string) => providers.find(p => p.id === id)?.models || [];
/** The High-level independent checker: Codex sources only, because the checker keeps and resumes its own Codex thread. */
function CheckerRow({ providers, value, enabled, change }: { providers: Provider[]; value: Mutis["high_checker"]; enabled: boolean; change: (patch: Partial<Mutis["high_checker"]>, immediate?: boolean) => void }) {
  return <div className={styles.row} aria-disabled={!enabled}><strong>{t.mutis.checkerRole}</strong>
    <label>{t.mutis.provider}<select disabled={!enabled} value={value.provider_id} onChange={event => change({ provider_id: event.target.value, model: modelsOf(providers, event.target.value)[0] || "" }, true)}>
      <option value="" />{providers.filter(p => p.kind === "codex_cli").map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
    <label>{t.mutis.model}<input disabled={!enabled} list="mutis-models-checker" value={value.model} onChange={event => change({ model: event.target.value })} />
      <datalist id="mutis-models-checker">{modelsOf(providers, value.provider_id).map(m => <option key={m} value={m} />)}</datalist></label>
    <label>{t.mutis.effort}<select disabled={!enabled} value={effortFor(value.model, value.effort, "codex_cli")} onChange={event => change({ effort: event.target.value as Mutis["high_checker"]["effort"] }, true)}>
      {effortsFor(value.model, "codex_cli").map(effort => <option key={effort} value={effort}>{t.efforts[effort]}</option>)}</select></label></div>;
}
export function MutisTab({ taskBoardEnabled, onTaskBoardEnabledChange }: { taskBoardEnabled: boolean; onTaskBoardEnabledChange: (enabled: boolean) => void }) {
  const [profile, setProfile] = useState<Profile>("high_roles");
  const client = useQueryClient();
  const { query: config } = useSettingsConfig();
  const query = useQuery({ queryKey: ["mutis"], queryFn: ({ signal }) => mutisApi.get(signal) });
  const autosave = useSettingsAutosave<Mutis>({ key: "ai-hub:settings:mutis", initial: query.data ?? MutisSchema.parse({ enabled: false }),
    validate: (value): value is Mutis => MutisSchema.safeParse(value).success,
    save: async value => { const saved = await mutisApi.put(value); client.setQueryData(["mutis"], saved); return saved; },
    onError: error => notify(errorText(error, t.error)) });
  const { draft, busy } = autosave;
  const hydrate = autosave.hydrate;
  useEffect(() => { if (query.data) hydrate(query.data); }, [query.data, hydrate]);
  function onProfileKey(event: React.KeyboardEvent<HTMLButtonElement>) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = PROFILES[(PROFILES.indexOf(profile) + step + PROFILES.length) % PROFILES.length];
    setProfile(next);
    document.getElementById(`mutis-tab-${next}`)?.focus();
  }
  if (query.isPending && !query.data) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const providers = config.data?.providers || [];
  const edit = (patch: Partial<Mutis>, immediate = false) => autosave.update(current => ({ ...current, ...patch }), immediate);
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void autosave.flush(); }}>
    <label className={styles.check}><Switch checked={taskBoardEnabled} onChange={event => onTaskBoardEnabledChange(event.target.checked)} /><span>{t.mutis.taskBoard}</span></label>
    <p>{t.mutis.levelHint}</p>
    <div className={styles.subTabs} role="tablist" aria-label={t.mutis.profileLabel}>
      {PROFILES.map(id => <button key={id} type="button" role="tab" id={`mutis-tab-${id}`} aria-selected={profile === id} aria-controls={`mutis-panel-${id}`}
        tabIndex={profile === id ? 0 : -1} onClick={() => setProfile(id)} onKeyDown={onProfileKey}>{id === "high_roles" ? "High" : "Max"}</button>)}
    </div>
    {PROFILES.map(id => <div key={id} role="tabpanel" id={`mutis-panel-${id}`} aria-labelledby={`mutis-tab-${id}`} hidden={profile !== id}>
      <fieldset className={styles.group}><legend>{id === "roles" ? t.mutis.maxRoles : t.mutis.highRoles}</legend>
        {ROLES.map(role => { const value = draft[id][role];
          const setRole = (patch: Partial<typeof value>, immediate = false) => autosave.update(current => ({
            ...current, [id]: { ...current[id], [role]: { ...current[id][role], ...patch } }
          }), immediate);
          return <div key={role} className={styles.row}><strong>{t.mutis.role[role]}</strong>
            <label>{t.mutis.provider}<select value={value.provider_id} onChange={event => setRole({ provider_id: event.target.value, model: modelsOf(providers, event.target.value)[0] || "" }, true)}>
              <option value="" />{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
            <label>{t.mutis.model}<input list={`mutis-models-${id}-${role}`} value={value.model} onChange={event => setRole({ model: event.target.value })} />
              <datalist id={`mutis-models-${id}-${role}`}>{modelsOf(providers, value.provider_id).map(m => <option key={m} value={m} />)}</datalist></label>
            <label>{t.mutis.effort}<select value={effortFor(value.model, value.effort, providers.find(p => p.id === value.provider_id)?.kind)} onChange={event => setRole({ effort: event.target.value as typeof value.effort }, true)}>
              {effortsFor(value.model, providers.find(p => p.id === value.provider_id)?.kind).map(effort => <option key={effort} value={effort}>{t.efforts[effort]}</option>)}</select></label></div>; })}
        {id === "high_roles" && <>
          <label className={styles.check}><Switch checked={draft.delegate_checker} onChange={event => edit({ delegate_checker: event.target.checked }, true)} /><span>{t.mutis.delegate_checker}</span></label>
          <CheckerRow providers={providers} value={draft.high_checker} enabled={draft.delegate_checker} change={(patch, immediate) => autosave.update(current => ({ ...current, high_checker: { ...current.high_checker, ...patch } }), immediate)} />
          <label className={styles.check}><Switch checked={draft.deepseek_ui} onChange={event => edit({ deepseek_ui: event.target.checked }, true)} /><span>{t.mutis.deepseek_ui}</span></label>
        </>}
      </fieldset>
    </div>)}
    <h3>{t.mutis.sharedSettings}</h3>
    <fieldset className={styles.group}><legend>{t.mutis.routing}</legend>
      <div className={styles.grid}>{draft.routing.map((rule, index) => <label key={rule.kind}>{t.mutis.kinds[rule.kind] || rule.kind}
        <select value={rule.role} onChange={event => autosave.update(current => ({
          ...current, routing: current.routing.map((item, i) => i === index ? { ...item, role: event.target.value } : item)
        }), true)}>
          {ROLES.map(role => <option key={role} value={role}>{t.mutis.role[role]}</option>)}</select></label>)}</div>
    </fieldset>
    <fieldset className={styles.group}><legend>{t.mutis.switches}</legend>
      {SWITCHES.map(item => <div key={item.key} className={styles.switch}>
        <label className={styles.check}><Switch checked={draft[item.key]} onChange={event => edit({ [item.key]: event.target.checked }, true)} /><span>{t.mutis[item.key]}</span></label>
        {item.numbers?.map(number => <label key={number.key}>{t.mutis[number.key]}<input type="number" min={number.min} max={number.max}
          value={draft[number.key]} disabled={!draft[item.key]} onChange={event => edit({ [number.key]: Number(event.target.value) })} /></label>)}
      </div>)}
      <label>{t.mutis.verify_command}<input value={draft.verify_command} placeholder={t.mutis.verify_commandHint} spellCheck={false}
        onChange={event => edit({ verify_command: event.target.value })} /></label>
      <VerifyProject />
      <label>{t.mutis.verify_ignore}<textarea rows={2} spellCheck={false} value={draft.verify_ignore.join("\n")}
        onChange={event => edit({ verify_ignore: event.target.value.split(/\r?\n/).map(x => x.trim()).filter(Boolean) })} /></label>
      <div className={styles.grid}>
        <label>{t.mutis.reviewer}<select value={draft.reviewer_provider_id} onChange={event => edit({ reviewer_provider_id: event.target.value, reviewer_model: modelsOf(providers, event.target.value)[0] || "" }, true)}>
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
