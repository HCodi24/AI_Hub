import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { verifyApi } from "@/apis/verify.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { useChatStore } from "@/components/features/chat/chat-store";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";

/** The verify command of the folder open in the chat: confirm what was detected, or write your own. */
export function VerifyProject() {
  const folder = useChatStore(state => state.settings.folder.trim());
  const client = useQueryClient();
  const key = ["verify-commands", folder];
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => verifyApi.get(folder, signal), enabled: Boolean(folder) });
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const data = query.data;
  useEffect(() => { if (data) setDraft(data.saved ?? ""); }, [data]);
  if (!folder) return <p className={styles.note}>{t.mutis.verifyProjectNoFolder}</p>;
  async function save(command: string, done: string) {
    setBusy(true);
    try { client.setQueryData(key, await verifyApi.put(folder, command)); notify(done); }
    catch (error) { notify(errorText(error, t.error)); }
    finally { setBusy(false); }
  }
  const detected = data?.detected.map(item => item.command).join(" && ") ?? "";
  return <div className={styles.switch}>
    <strong>{t.mutis.verifyProject}</strong>
    <p className={styles.note}>{folder}</p>
    {query.error && <p role="alert">{errorText(query.error, t.error)}</p>}
    {data && (data.detected.length
      ? <p className={styles.note}>{t.mutis.verifyDetected}: {data.detected.map(item => <code key={item.command}>{item.command} </code>)}</p>
      : <p className={styles.note}>{t.mutis.verifyNone}</p>)}
    <label>{t.mutis.verifyProjectCommand}<input value={draft} placeholder={t.mutis.verifyProjectHint} spellCheck={false} disabled={busy || !data}
      onChange={event => setDraft(event.target.value)} /></label>
    <div className={styles.actions}>
      <Button type="button" disabled={busy || !data || draft.trim() === (data.saved ?? "")} onClick={() => void save(draft, t.mutis.verifyProjectSaved)}>{t.save}</Button>
      {detected && detected !== data?.saved && <Button type="button" disabled={busy} onClick={() => void save(detected, t.mutis.verifyProjectSaved)}>{t.mutis.verifyConfirmDetected}</Button>}
      {data?.saved && <Button type="button" disabled={busy} onClick={() => void save("", t.mutis.verifyProjectRemoved)}>{t.mutis.verifyClear}</Button>}
    </div>
  </div>;
}
