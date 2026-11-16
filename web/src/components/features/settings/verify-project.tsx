import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { verifyApi } from "@/apis/verify.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { useChatStore } from "@/components/features/chat/chat-store";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
import { useSettingsAutosave } from "./use-settings-autosave";

/** The verify command of the folder open in the chat: confirm what was detected, or write your own. */
export function VerifyProject() {
  const folder = useChatStore(state => state.settings.folder.trim());
  if (!folder) return <p className={styles.note}>{t.mutis.verifyProjectNoFolder}</p>;
  return <VerifyProjectFolder key={folder} folder={folder} />;
}

function VerifyProjectFolder({ folder }: { folder: string }) {
  const client = useQueryClient();
  const key = ["verify-commands", folder];
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => verifyApi.get(folder, signal), enabled: Boolean(folder) });
  const data = query.data;
  const autosave = useSettingsAutosave<string>({ key: `ai-hub:settings:verify:${folder}`, initial: data?.saved ?? "",
    validate: (value): value is string => typeof value === "string",
    save: async command => { const result = await verifyApi.put(folder, command); client.setQueryData(key, result); return result.saved ?? ""; },
    onError: error => notify(errorText(error, t.error)) });
  const { draft, busy } = autosave;
  const hydrate = autosave.hydrate;
  useEffect(() => { if (data) hydrate(data.saved ?? ""); }, [data, hydrate]);
  const detected = data?.detected.map(item => item.command).join(" && ") ?? "";
  async function save(command: string, done: string) {
    autosave.schedule(command, true);
    if (await autosave.flush()) notify(done);
  }
  return <div className={styles.switch}>
    <strong>{t.mutis.verifyProject}</strong>
    <p className={styles.note}>{folder}</p>
    {query.error && <p role="alert">{errorText(query.error, t.error)}</p>}
    {data && (data.detected.length
      ? <p className={styles.note}>{t.mutis.verifyDetected}: {data.detected.map(item => <code key={item.command}>{item.command} </code>)}</p>
      : <p className={styles.note}>{t.mutis.verifyNone}</p>)}
    <label>{t.mutis.verifyProjectCommand}<input value={draft} placeholder={t.mutis.verifyProjectHint} spellCheck={false} disabled={busy || !data}
      onChange={event => autosave.schedule(event.target.value)} /></label>
    <div className={styles.actions}>
      <Button type="button" disabled={busy || !data || draft.trim() === (data.saved ?? "")} onClick={() => void save(draft, t.mutis.verifyProjectSaved)}>{t.save}</Button>
      {detected && detected !== data?.saved && <Button type="button" disabled={busy} onClick={() => void save(detected, t.mutis.verifyProjectSaved)}>{t.mutis.verifyConfirmDetected}</Button>}
      {data?.saved && <Button type="button" disabled={busy} onClick={() => void save("", t.mutis.verifyProjectRemoved)}>{t.mutis.verifyClear}</Button>}
    </div>
  </div>;
}
