import { useEffect, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { mutisApi } from "@/apis/mutis.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { Switch } from "@/components/button/switch";
import { Dropdown } from "@/components/select/select";
import { notify } from "@/components/toast/toast";
import { MutisSchema, type Mutis } from "@/schema/mutis.schema";
import type { Provider } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { MutisDiagnosis } from "./mutis-diagnosis";
import { MutisStats } from "./mutis-stats";
import { VerifyProject } from "./verify-project";
import { errorText, useSettingsConfig } from "../use-settings";
import styles from "../settings.module.css";
import { useSettingsAutosave } from "../use-settings-autosave";
import { roleEffortOptions } from "@/constants/chat";
import { antigravityModel, antigravityVariants, ANTIGRAVITY_EFFORTS } from "@/lib/antigravity-models";
const DEFAULT_SETTINGS = MutisSchema.parse({});
type RoleKey = "lead" | "worker" | "checker";
type Permission = Mutis["worker"]["permission"];
type BoolKey = "verify_gate" | "auto_review" | "auto_fix" | "retry_api" | "codex_watchdog" | "codex_auto_resume" | "deepseek_agent" | "event_log";
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
/** Model options for a select: the provider's models, plus the saved value when it is not in that list so it is never lost. */
const modelOptions = (models: string[], current: string) => {
  return <>{current && !models.includes(current) && <option value={current}>{current}</option>}{!current && <option value="">{t.mutis.modelPlaceholder}</option>}
    {models.map(m => <option key={m} value={m}>{m}</option>)}</>;
};
const effortLabel = (effort: keyof typeof t.efforts | "thinking") => effort === "thinking" ? "Suy luận" : t.efforts[effort];
function RoleRow({ title, control, providers, value, disabled = false, change, children }: {
  title: string;
  control?: ReactNode;
  providers: Provider[];
  value: Mutis["lead"];
  disabled?: boolean;
  change: (patch: Partial<Mutis["lead"]>) => void;
  children: ReactNode;
}) {
  const provider = providers.find(provider => provider.id === value.provider_id);
  const groups = provider?.kind === "antigravity_cli" ? antigravityVariants(provider.models) : undefined;
  const model = groups ? antigravityModel(value.model) : undefined;
  const selectedModel = model?.id ?? value.model;
  const variants = groups?.get(selectedModel);
  const standardEfforts = roleEffortOptions(provider, value.model, value.effort);
  const effortChoices = groups
    ? (["default", ...ANTIGRAVITY_EFFORTS] as const).filter(effort => variants?.has(effort))
    : standardEfforts.choices;
  const selectedEffort = groups ? model?.effort ?? "default" : standardEfforts.selected;
  return <div className={styles.row} aria-disabled={disabled || undefined}>
    <div className={styles.roleCell}><strong>{title}</strong>{control}</div>
    <label><span className="sr-only">{t.mutis.provider}</span><Dropdown disabled={disabled} data-empty={!value.provider_id || undefined} value={value.provider_id} onChange={event => change({ provider_id: event.target.value, model: modelsOf(providers, event.target.value)[0] || "" })}>
      <option value="">{t.mutis.providerPlaceholder}</option>{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</Dropdown></label>
    <label><span className="sr-only">{t.mutis.model}</span><Dropdown disabled={disabled} data-empty={!value.model || undefined} value={selectedModel}
      title={groups && value.model && !provider?.models.includes(value.model) ? value.model : undefined}
      onChange={event => {
        if (!groups) { change({ model: event.target.value }); return; }
        const next = groups.get(event.target.value);
        const variant = next?.get(model?.effort ?? "default") ?? next?.values().next().value;
        if (variant) change({ model: variant, effort: "default" });
      }}>
      {modelOptions(groups ? [...groups.keys()] : provider?.models ?? [], selectedModel)}</Dropdown></label>
    <label><span className="sr-only">{t.mutis.effort}</span><Dropdown disabled={disabled} value={selectedEffort} onChange={event => {
      if (groups) {
        const effort = (["default", ...ANTIGRAVITY_EFFORTS] as const).find(choice => choice === event.target.value);
        const variant = effort && variants?.get(effort);
        if (variant) change({ model: variant, effort: "default" });
      } else {
        const effort = standardEfforts.choices.find(choice => choice === event.target.value);
        if (effort) change({ effort });
      }
    }}>
      {groups && !effortChoices.some(effort => effort === selectedEffort) &&
        <option value={selectedEffort} hidden disabled>{value.model ? effortLabel(selectedEffort) : ""}</option>}
      {effortChoices.map(effort => <option key={effort} value={effort}>{effortLabel(effort)}</option>)}</Dropdown></label>
    <div className={styles.roleSubRow}>{children}</div>
  </div>;
}
function RolePermission({ value, disabled, className, change }: { value: Permission; disabled?: boolean; className?: string; change: (permission: Permission) => void }) {
  return <label className={className}>{t.mutis.permission}<Dropdown value={value} disabled={disabled} onChange={event => change(event.target.value as Permission)}>
    {Object.entries(t.mutis.permissions).map(([permission, label]) => <option key={permission} value={permission}>{label}</option>)}
  </Dropdown></label>;
}
export function MutisTab() {
  const client = useQueryClient();
  const { query: config } = useSettingsConfig();
  const query = useQuery({ queryKey: ["mutis"], queryFn: ({ signal }) => mutisApi.get(signal) });
  const autosave = useSettingsAutosave<Mutis>({ key: "ai-hub:settings:mutis", initial: query.data ?? DEFAULT_SETTINGS,
    validate: (value): value is Mutis => MutisSchema.safeParse(value).success,
    save: async value => { const saved = await mutisApi.put(value); client.setQueryData(["mutis"], saved); return saved; },
    onError: error => notify(errorText(error, t.error)) });
  const { draft, busy } = autosave;
  const hydrate = autosave.hydrate;
  useEffect(() => { if (query.data) hydrate(query.data); }, [query.data, hydrate]);
  if (query.isPending && !query.data) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const providers = config.data?.providers || [];
  const reviewerProvider = providers.find(provider => provider.id === draft.reviewer_provider_id);
  const reviewerGroups = reviewerProvider?.kind === "antigravity_cli" ? antigravityVariants(reviewerProvider.models) : undefined;
  const reviewerModel = reviewerGroups ? antigravityModel(draft.reviewer_model).id : draft.reviewer_model;
  const edit = (patch: Partial<Mutis>, immediate = false) => autosave.update(current => ({ ...current, ...patch }), immediate);
  const setRole = <K extends RoleKey>(role: K, patch: Partial<Mutis[K]>) => autosave.update(current => ({
    ...current, [role]: { ...(current[role] ?? DEFAULT_SETTINGS[role]), ...patch }
  }), true);
  // Bản nháp lưu từ phiên bản cũ có thể thiếu ba vai trò mới.
  const lead = draft.lead ?? DEFAULT_SETTINGS.lead;
  const worker = draft.worker ?? DEFAULT_SETTINGS.worker;
  const checker = draft.checker ?? DEFAULT_SETTINGS.checker;
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void autosave.flush(); }}>
    <div className={styles.roleTable}>
      <div className={`${styles.row} ${styles.roleHead}`} aria-hidden="true"><span>{t.mutis.role}</span><span>{t.mutis.provider}</span><span>{t.mutis.model}</span><span>{t.mutis.effort}</span></div>
      <RoleRow title={t.mutis.leadRole} providers={providers} value={lead} change={patch => setRole("lead", patch)}>
        <p className={styles.roleNote}>{t.mutis.leadPermissionNote}</p>
      </RoleRow>
      <RoleRow title={t.mutis.workerRole} providers={providers} value={worker} change={patch => setRole("worker", patch)}>
        <RolePermission className={styles.rolePermSpan} value={worker.permission} change={permission => setRole("worker", { permission })} />
        <label>{t.mutis.workerCount}<Dropdown value={worker.count} onChange={event => setRole("worker", { count: Number(event.target.value) })}>
          {[1, 2, 3].map(count => <option key={count} value={count}>{count}</option>)}
        </Dropdown></label>
        <p className={styles.roleNote}>{t.mutis.workerPermissionNote(worker.count)}</p>
      </RoleRow>
      <RoleRow
        title={t.mutis.checkerRole}
        control={<label className={styles.roleToggle}>
          <Switch checked={draft.delegate_checker} aria-label={t.mutis.delegate_checker} onChange={event => edit({ delegate_checker: event.target.checked }, true)} />
          <span>{draft.delegate_checker ? t.mutis.checkerOn : t.mutis.checkerOff}</span>
        </label>}
        providers={providers}
        value={checker}
        disabled={!draft.delegate_checker}
        change={patch => setRole("checker", patch)}
      >
        <RolePermission className={styles.rolePermSpan} value={checker.permission} disabled={!draft.delegate_checker} change={permission => setRole("checker", { permission })} />
        <p className={styles.roleNote}>{draft.delegate_checker ? t.mutis.checkerActiveNote : t.mutis.checkerDisabledNote}</p>
      </RoleRow>
    </div>
    <h3>{t.mutis.sharedSettings}</h3>
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
        <label>{t.mutis.reviewer}<Dropdown value={draft.reviewer_provider_id} onChange={event => edit({ reviewer_provider_id: event.target.value, reviewer_model: modelsOf(providers, event.target.value)[0] || "" }, true)}>
          <option value="" />{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</Dropdown></label>
        <label>{t.mutis.reviewerModel}<Dropdown value={reviewerModel}
          title={reviewerGroups && draft.reviewer_model && !reviewerProvider?.models.includes(draft.reviewer_model) ? draft.reviewer_model : undefined}
          onChange={event => {
            const model = reviewerGroups ? reviewerGroups.get(event.target.value)?.values().next().value : event.target.value;
            if (model !== undefined) edit({ reviewer_model: model }, true);
          }}>
          {modelOptions(reviewerGroups ? [...reviewerGroups.keys()] : reviewerProvider?.models ?? [], reviewerModel)}</Dropdown></label>
      </div>
    </fieldset>
    <fieldset className={styles.group}><legend>{t.mutis.debate}</legend><div className={styles.grid}>
      <label>{t.mutis.debate_rounds}<input type="number" min={1} max={3} value={draft.debate_rounds} onChange={event => edit({ debate_rounds: Number(event.target.value) })} /></label>
      <label>{t.mutis.debate_judge_model}<input value={draft.debate_judge_model} placeholder={t.mutis.debateJudgeHint} onChange={event => edit({ debate_judge_model: event.target.value })} /></label>
    </div></fieldset>
    <fieldset className={styles.group}><legend>{t.mutis.rulesByRole}</legend>
      <p>{t.mutis.rulesHint}</p>
      <label>{t.mutis.lead_rules}<textarea rows={4} value={draft.lead_rules} onChange={event => edit({ lead_rules: event.target.value })} /></label>
      <label className={styles.check}><Switch checked={draft.lead_app_rules} onChange={event => edit({ lead_app_rules: event.target.checked }, true)} /><span>{t.mutis.lead_app_rules}</span></label>
      <label>{t.mutis.worker_rules}<textarea rows={5} value={draft.worker_rules} onChange={event => edit({ worker_rules: event.target.value })} /></label>
      <label>{t.mutis.checker_rules}<textarea rows={4} value={draft.checker_rules} onChange={event => edit({ checker_rules: event.target.value })} /></label>
      <label className={styles.check}><Switch checked={draft.checker_app_rules} onChange={event => edit({ checker_app_rules: event.target.checked }, true)} /><span>{t.mutis.checker_app_rules}</span></label>
    </fieldset>
    <div className={styles.actions}><Button type="submit" variant="primary" disabled={busy}>{t.save}</Button></div>
    <MutisDiagnosis />
    <MutisStats />
  </form>;
}
