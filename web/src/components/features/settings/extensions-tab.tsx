import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink } from "lucide-react";
import { extensionsApi } from "@/apis/extensions.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { Extension, ExtensionAction } from "@/schema/extensions.schema";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
/** Which actions make sense for the extension's current state (same rules as the legacy UI). */
function visible(card: Extension, action: ExtensionAction, folder: string) {
  if (action.id === "install") return !card.installed;
  if (action.id === "uninstall" || action.id === "upgrade") return card.installed;
  if (action.id === "connect_codex") return card.installed && !card.codex;
  if (action.id === "connect_claude") return card.installed && !card.claude;
  if (action.id === "index_folder") return card.installed && Boolean(folder);
  return true;
}
const text = (data: unknown) => typeof data === "string" ? data
  : typeof data === "object" && data && "message" in data ? String((data as { message: unknown }).message) : "";
export function ExtensionsTab({ folder }: { folder: string }) {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ["extensions", folder], queryFn: ({ signal }) => extensionsApi.list(folder, signal) });
  const [pending, setPending] = useState<{ card: Extension; action: ExtensionAction } | null>(null);
  const [running, setRunning] = useState(""); const [log, setLog] = useState("");
  const abort = useRef<AbortController | null>(null);
  const command = (action: ExtensionAction) => `${action.command}${action.id === "index_folder" ? `  (cwd: ${folder})` : ""}`;
  async function run(card: Extension, action: ExtensionAction) {
    setPending(null); setRunning(card.id); setLog(current => `${current}\n> ${command(action)}\n`);
    abort.current = new AbortController();
    try {
      await extensionsApi.run(card.id, action.id, folder, abort.current.signal, (event, data) => {
        if (event === "log") setLog(current => `${current}${text(data)}\n`);
        if (event === "error") { const message = text(data) || t.extensions.failed; notify(message); setLog(current => `${current}${message}\n`); }
        if (event === "done" && typeof data === "object" && data && "ok" in data && data.ok) notify(t.extensions.done);
      });
    } catch (error) { const message = errorText(error, t.error); notify(message); setLog(current => `${current}${message}\n`); }
    finally { setRunning(""); await client.invalidateQueries({ queryKey: ["extensions"] }); }
  }
  return <div className={styles.form}>
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {list.data?.map(card => <article key={card.id} className={styles.group}>
      <header className={styles.cardHead}><h3>{card.name}</h3>
        <span className={styles.badge} data-on={card.installed}>{card.installed ? `${t.extensions.installed}${card.version ? ` · ${card.version}` : ""}` : t.extensions.notInstalled}</span>
        {card.installed && <span className={styles.badge} data-on={card.codex}>{t.extensions.codex}</span>}
        {card.installed && <span className={styles.badge} data-on={card.claude}>{t.extensions.claude}</span>}
        {card.folder_indexed && <span className={styles.badge} data-on="true">{t.extensions.indexed}</span>}</header>
      <p>{card.description}</p>
      {card.homepage && <a href={card.homepage} target="_blank" rel="noreferrer">{t.extensions.homepage} <ExternalLink size={14} aria-hidden="true" /></a>}
      <div className={styles.actions}>{card.actions.filter(action => visible(card, action, folder)).map(action =>
        <Button key={action.id} variant={action.id === "uninstall" ? "danger" : "secondary"} disabled={Boolean(running)}
          onClick={() => setPending({ card, action })}>{action.label}</Button>)}
        {running === card.id && <Button variant="danger" onClick={() => { abort.current?.abort(); void extensionsApi.stop(card.id).catch(() => undefined); }}>{t.extensions.stop}</Button>}</div>
      {pending?.card.id === card.id && <div className={styles.confirm} role="group">
        <p className={styles.pre}>{t.extensions.confirm(pending.action.label, command(pending.action), pending.action.modifies)}</p>
        <Button variant="primary" onClick={() => void run(card, pending.action)}>{t.extensions.run}</Button>
        <Button onClick={() => setPending(null)}>{t.cancel}</Button></div>}
    </article>)}
    <section aria-label={t.extensions.log}><h3>{t.extensions.log}</h3>
      <pre className={styles.log} aria-live="polite">{log.trim() || t.extensions.empty}</pre></section>
  </div>;
}
