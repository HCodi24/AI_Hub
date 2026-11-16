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
type GeneralDraft = { folder: string; idle: number; memory: MemoryConfig };
const isGeneralDraft = (value: unknown): value is GeneralDraft => {
  if (!value || typeof value !== "object") return false;
  const item = value as GeneralDraft;
  return typeof item.folder === "string" && typeof item.idle === "number" && Number.isFinite(item.idle) &&
    !!item.memory && typeof item.memory.enabled === "boolean" && Number.isFinite(item.memory.max_scopes_in_prompt) && Number.isFinite(item.memory.max_chars_per_scope);
};
export function GeneralTab() {
  const { query, apply } = useSettingsConfig();
  const initial: GeneralDraft = { folder: query.data?.default_folder ?? "", idle: query.data?.chat_idle_timeout_minutes ?? 10,
    memory: query.data?.memory ?? { enabled: true, max_scopes_in_prompt: 3, max_chars_per_scope: 6000 } };
  const autosave = useSettingsAutosave<GeneralDraft>({ key: "ai-hub:settings:general", initial, validate: isGeneralDraft,
    save: async value => {
      const saved = await settingsApi.saveConfig({ default_folder: value.folder.trim(), chat_idle_timeout_minutes: clamp(value.idle, 0, 1440),
        memory: { enabled: value.memory.enabled, max_scopes_in_prompt: clamp(value.memory.max_scopes_in_prompt, 1, 10),
          max_chars_per_scope: clamp(value.memory.max_chars_per_scope, 1000, 20000) } });
      apply(saved); return value;
    }, onError: error => notify(errorText(error, t.error)) });
  const { draft, busy } = autosave;
  const hydrate = autosave.hydrate;
  const data = query.data;
  useEffect(() => { if (data) hydrate({ folder: data.default_folder, idle: data.chat_idle_timeout_minutes, memory: data.memory }); }, [data, hydrate]);
  if (query.isPending) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void autosave.flush(); }}>
    <label>{t.general.folder}<input required value={draft.folder} spellCheck={false} onChange={event => autosave.update(current => ({ ...current, folder: event.target.value }))} /></label>
    <label>{t.general.idle}<input type="number" min={0} max={1440} value={draft.idle} onChange={event => autosave.update(current => ({ ...current, idle: Number(event.target.value) }))} /></label>
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
