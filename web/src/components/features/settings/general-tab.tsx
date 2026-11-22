import { useEffect } from "react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { Switch } from "@/components/button/switch";
import { notify } from "@/components/toast/toast";
import type { MemoryConfig } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { errorText, useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
import { useSettingsAutosave } from "./use-settings-autosave";
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, Number.isFinite(value) ? value : min));
type GeneralDraft = { idle: number; threadBudget: number; approvalWait: number; memory: MemoryConfig };
/** 0 turns the rotation off; anything else stays inside what the server accepts. */
const budgetFor = (value: number) => (Number.isFinite(value) && value > 0 ? clamp(value, 20000, 2000000) : 0);
const isGeneralDraft = (value: unknown): value is GeneralDraft => {
  if (!value || typeof value !== "object") return false;
  const item = value as GeneralDraft;
  return typeof item.idle === "number" && Number.isFinite(item.idle) &&
    typeof item.threadBudget === "number" && Number.isFinite(item.threadBudget) &&
    typeof item.approvalWait === "number" && Number.isFinite(item.approvalWait) &&
    !!item.memory && typeof item.memory.enabled === "boolean" && Number.isFinite(item.memory.max_scopes_in_prompt) && Number.isFinite(item.memory.max_chars_per_scope);
};
export function GeneralTab({ taskBoardEnabled, onTaskBoardEnabledChange }: { taskBoardEnabled: boolean; onTaskBoardEnabledChange: (enabled: boolean) => void }) {
  const { query, apply } = useSettingsConfig();
  const initial: GeneralDraft = { idle: query.data?.chat_idle_timeout_minutes ?? 10,
    threadBudget: query.data?.thread_token_budget ?? 150000, approvalWait: query.data?.approval_wait_seconds ?? 120,
    memory: query.data?.memory ?? { enabled: true, max_scopes_in_prompt: 3, max_chars_per_scope: 6000 } };
  const autosave = useSettingsAutosave<GeneralDraft>({ key: "ai-hub:settings:general", initial, validate: isGeneralDraft,
    save: async value => {
      const saved = await settingsApi.saveConfig({ chat_idle_timeout_minutes: clamp(value.idle, 0, 1440), thread_token_budget: budgetFor(value.threadBudget), approval_wait_seconds: clamp(value.approvalWait, 10, 3600),
        memory: { enabled: value.memory.enabled, max_scopes_in_prompt: clamp(value.memory.max_scopes_in_prompt, 1, 10),
          max_chars_per_scope: clamp(value.memory.max_chars_per_scope, 1000, 20000) } });
      apply(saved); return value;
    }, onError: error => notify(errorText(error, t.error)) });
  const { draft, busy } = autosave;
  const hydrate = autosave.hydrate;
  const data = query.data;
  useEffect(() => { if (data) hydrate({ idle: data.chat_idle_timeout_minutes, threadBudget: data.thread_token_budget, approvalWait: data.approval_wait_seconds, memory: data.memory }); }, [data, hydrate]);
  if (query.isPending) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void autosave.flush(); }}>
    <label>{t.general.idle}<input type="number" min={0} max={1440} value={draft.idle} onChange={event => autosave.update(current => ({ ...current, idle: Number(event.target.value) }))} /></label>
    <label>{t.general.threadBudget}<input type="number" min={0} max={2000000} step={10000} value={draft.threadBudget} onChange={event => autosave.update(current => ({ ...current, threadBudget: Number(event.target.value) }))} /></label>
    <label>{t.general.approvalWait}<input type="number" min={10} max={3600} step={10} value={draft.approvalWait} onChange={event => autosave.update(current => ({ ...current, approvalWait: Number(event.target.value) }))} /></label>
    <label className={styles.check}><Switch checked={taskBoardEnabled} onChange={event => onTaskBoardEnabledChange(event.target.checked)} /><span>{t.mutis.taskBoard}</span></label>
    <fieldset className={styles.group}><legend>{t.general.memory}</legend>
      <label className={styles.check}><Switch checked={draft.memory.enabled}
        onChange={event => autosave.update(current => ({ ...current, memory: { ...current.memory, enabled: event.target.checked } }), true)} /><span>{t.general.memoryEnabled}</span></label>
      <label>{t.general.memoryScopes}<input type="number" min={1} max={10} value={draft.memory.max_scopes_in_prompt} disabled={!draft.memory.enabled}
        onChange={event => autosave.update(current => ({ ...current, memory: { ...current.memory, max_scopes_in_prompt: Number(event.target.value) } }))} /></label>
      <label>{t.general.memoryChars}<input type="number" min={1000} max={20000} step={500} value={draft.memory.max_chars_per_scope} disabled={!draft.memory.enabled}
        onChange={event => autosave.update(current => ({ ...current, memory: { ...current.memory, max_chars_per_scope: Number(event.target.value) } }))} /></label>
    </fieldset>
    <div className={styles.actions}><Button type="submit" variant="primary" disabled={busy}>{t.save}</Button></div>
  </form>;
}
