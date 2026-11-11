import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { memoryApi } from "@/apis/memory.api";
import { Dialog } from "@/components/dialog/dialog";
import { notify } from "@/components/toast/toast";
import { cardText as t } from "@/text/cards";
import styles from "./memory-panel.module.css";
export function MemoryPanel({ folder, close }: { folder: string; close: () => void }) {
  const client = useQueryClient();
  const view = useQuery({ queryKey: ["memory", folder], queryFn: ({ signal }) => memoryApi.overview(folder, signal) });
  const [selected, setSelected] = useState<{ scope: string; archived: boolean } | null>(null);
  const [text, setText] = useState(""); const [busy, setBusy] = useState(false); const [confirm, setConfirm] = useState<"archive" | "restore" | "delete" | null>(null);
  async function run(action: () => Promise<unknown>, refresh = false) {
    setBusy(true);
    try { await action(); if (refresh) { await client.invalidateQueries({ queryKey: ["memory", folder] }); setSelected(null); setConfirm(null); } }
    catch (error) { notify(error instanceof Error ? error.message : t.error); } finally { setBusy(false); }
  }
  async function open(scope: string, archived: boolean) {
    await run(async () => { const result = await memoryApi.get(folder, scope); setSelected({ scope, archived }); setText(result.text); setConfirm(null); });
  }
  return <Dialog title={t.memoryTitle} close={close}><p>{folder}</p>
    {view.error && <p role="alert">{view.error.message}</p>}
    <div className={styles.scopes}>{([false, true] as const).map(archived => <section key={String(archived)}><h3>{archived ? t.archived : t.activeScopes}</h3>
      {(archived ? view.data?.archived : view.data?.scopes)?.map(scope => <button key={scope.scope} disabled={busy} aria-pressed={selected?.scope === scope.scope} onClick={() => void open(scope.scope, archived)}>[{scope.status}] {scope.scope} — {scope.title}</button>)}</section>)}</div>
    {selected && <section><label>{t.scope}<textarea value={text} disabled={busy} onChange={event => setText(event.target.value)} /></label>
      <div className={styles.actions}><button disabled={busy} onClick={() => void run(async () => { await memoryApi.put(folder, selected.scope, text); notify(t.saved); }, true)}>{t.save}</button>
        <button disabled={busy} onClick={() => setConfirm(selected.archived ? "restore" : "archive")}>{selected.archived ? t.restore : t.archive}</button>
        {selected.archived && <button disabled={busy} onClick={() => setConfirm("delete")}>{t.delete}</button>}</div>
      {confirm && <div role="group" aria-label={t.confirmMemory(t[confirm], selected.scope)}><p>{t.confirmMemory(t[confirm], selected.scope)}</p>
        <button disabled={busy} onClick={() => void run(() => memoryApi[confirm](folder, selected.scope), true)}>{t[confirm]}</button><button onClick={() => setConfirm(null)}>{t.cancel}</button></div>}
    </section>}</Dialog>;
}
