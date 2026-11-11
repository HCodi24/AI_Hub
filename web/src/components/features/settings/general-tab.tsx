import { useEffect, useState } from "react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { MemoryConfig } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { errorText, useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, Number.isFinite(value) ? value : min));
export function GeneralTab() {
  const { query, apply } = useSettingsConfig();
  const [folder, setFolder] = useState(""); const [idle, setIdle] = useState(10);
  const [memory, setMemory] = useState<MemoryConfig>({ enabled: true, max_scopes_in_prompt: 3, max_chars_per_scope: 6000 });
  const [busy, setBusy] = useState(false);
  const data = query.data;
  useEffect(() => {
    if (!data) return;
    setFolder(data.default_folder); setIdle(data.chat_idle_timeout_minutes); setMemory(data.memory);
  }, [data]);
  if (query.isPending) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  async function save() {
    setBusy(true);
    try {
      apply(await settingsApi.saveConfig({ default_folder: folder.trim(), chat_idle_timeout_minutes: clamp(idle, 0, 1440),
        memory: { enabled: memory.enabled, max_scopes_in_prompt: clamp(memory.max_scopes_in_prompt, 1, 10),
          max_chars_per_scope: clamp(memory.max_chars_per_scope, 1000, 20000) } }));
      notify(t.saved);
    } catch (error) { notify(errorText(error, t.error)); } finally { setBusy(false); }
  }
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void save(); }}>
    <label>{t.general.folder}<input required value={folder} spellCheck={false} onChange={event => setFolder(event.target.value)} /></label>
    <label>{t.general.idle}<input type="number" min={0} max={1440} value={idle} onChange={event => setIdle(Number(event.target.value))} /></label>
    <fieldset className={styles.group}><legend>{t.general.memory}</legend>
      <label className={styles.check}><input type="checkbox" checked={memory.enabled}
        onChange={event => setMemory({ ...memory, enabled: event.target.checked })} /><span>{t.general.memoryEnabled}</span></label>
      <label>{t.general.memoryScopes}<input type="number" min={1} max={10} value={memory.max_scopes_in_prompt} disabled={!memory.enabled}
        onChange={event => setMemory({ ...memory, max_scopes_in_prompt: Number(event.target.value) })} /></label>
      <label>{t.general.memoryChars}<input type="number" min={1000} max={20000} step={500} value={memory.max_chars_per_scope} disabled={!memory.enabled}
        onChange={event => setMemory({ ...memory, max_chars_per_scope: Number(event.target.value) })} /></label>
    </fieldset>
    <div className={styles.actions}><Button type="submit" variant="primary" disabled={busy}>{t.save}</Button></div>
  </form>;
}
