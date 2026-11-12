import { FileEdit, Paperclip } from "lucide-react";
import { Skeleton } from "@/components/skeleton/skeleton";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { memoryApi } from "@/apis/memory.api";
import { Dialog } from "@/components/dialog/dialog";
import { notify } from "@/components/toast/toast";
import { cardText as t } from "@/text/cards";
import { folderName } from "@/lib/utils";
import styles from "./memory-panel.module.css";
/** `attach` adds recalled memory to the next message; nothing is read by the AI unless the user picks it here. */
export function MemoryPanel({ folder, close, attach }: { folder: string; close: () => void; attach: (label: string, text: string) => void }) {
  const client = useQueryClient();
  const view = useQuery({ queryKey: ["memory", folder], queryFn: ({ signal }) => memoryApi.overview(folder, signal) });
  const projects = useQuery({ queryKey: ["memory-projects"], queryFn: ({ signal }) => memoryApi.projects(signal) });
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
  function recall(label: string, target: Parameters<typeof memoryApi.recall>[0]) {
    void run(async () => { attach(label, await memoryApi.recall(target)); notify(t.attached(label)); close(); });
  }
  const others = projects.data?.filter(project => project.path.toLowerCase() !== folder.toLowerCase()) || [];
  return <Dialog title={t.memoryTitle} close={close}><p><FileEdit size={16} aria-hidden="true" /> {folder}</p>
    {view.isPending && <Skeleton card />}
    {view.error && <p role="alert">{view.error.message}</p>}
    <div className={styles.scopes}>{([false, true] as const).map(archived => <section key={String(archived)}><h3>{archived ? t.archived : t.activeScopes}</h3>
      {(archived ? view.data?.archived : view.data?.scopes)?.map(scope => <div key={scope.scope} className={styles.row}>
        <button disabled={busy} aria-pressed={selected?.scope === scope.scope} onClick={() => void open(scope.scope, archived)}>[{scope.status}] {scope.title || scope.scope}</button>
        <button disabled={busy} aria-label={`${t.attachMemory}: ${scope.title || scope.scope}`} title={t.attachMemory}
          onClick={() => recall(scope.title || scope.scope, { folder, session: scope.scope })}><Paperclip size={16} aria-hidden="true" /></button></div>)}</section>)}</div>
    {others.length > 0 && <section><h3>{t.otherProjects}</h3><div className={styles.scopes}>{others.map(project => <div key={project.slug} className={styles.row}>
      <span title={project.path}>{folderName(project.path)}{project.description && ` — ${project.description}`}</span>
      <button disabled={busy} aria-label={`${t.attachMemory}: ${folderName(project.path)}`} title={t.attachMemory}
        onClick={() => recall(folderName(project.path), { project: project.slug })}><Paperclip size={16} aria-hidden="true" /></button></div>)}</div></section>}
    {selected && <section><label>{t.scope}<textarea value={text} disabled={busy} onChange={event => setText(event.target.value)} /></label>
      <div className={styles.actions}><button disabled={busy} onClick={() => void run(async () => { await memoryApi.put(folder, selected.scope, text); notify(t.saved); }, true)}>{t.save}</button>
        <button disabled={busy} onClick={() => setConfirm(selected.archived ? "restore" : "archive")}>{selected.archived ? t.restore : t.archive}</button>
        {selected.archived && <button disabled={busy} onClick={() => setConfirm("delete")}>{t.delete}</button>}</div>
      {confirm && <div role="group" aria-label={t.confirmMemory(t[confirm], selected.scope)}><p>{t.confirmMemory(t[confirm], selected.scope)}</p>
        <button disabled={busy} onClick={() => void run(() => memoryApi[confirm](folder, selected.scope), true)}>{t[confirm]}</button><button onClick={() => setConfirm(null)}>{t.cancel}</button></div>}
    </section>}</Dialog>;
}
