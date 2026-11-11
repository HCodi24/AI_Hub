import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { devApi } from "@/apis/dev.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { DevDiff } from "@/schema/dev.schema";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
const field = (data: unknown, key: string): unknown => typeof data === "object" && data !== null ? (data as Record<string, unknown>)[key] : undefined;
/** One output line for an event of the dev test / release / rebuild streams. */
function line(event: string, data: unknown): string {
  if (event === "log") return String(field(data, "text") ?? (typeof data === "string" ? data : ""));
  if (event === "summary" && field(data, "web_ok") !== undefined) return field(data, "web_ok") ? t.dev.webOk : t.dev.webFail;
  if (event === "summary" && field(data, "cargo_ok") !== undefined) return field(data, "cargo_ok") ? t.dev.cargoOk : t.dev.cargoFail;
  if (event === "summary") return field(data, "ok") ? `${t.dev.buildOk}\n${String(field(data, "exe") ?? "")}\n${String(field(data, "note") ?? "")}` : t.dev.buildFailed;
  if (event === "verify") { const lines = field(data, "summary_lines"); return Array.isArray(lines) ? lines.join(" · ") : ""; }
  if (event === "rebuild") return field(data, "ok") ? t.dev.restarting : `${t.dev.buildFailed}:\n${String(field(data, "errors_tail") ?? "")}`;
  return "";
}
export function DevTab() {
  const client = useQueryClient();
  const status = useQuery({ queryKey: ["dev-status"], queryFn: ({ signal }) => devApi.status(signal) });
  const on = status.data?.dev === true;
  const repo = useQuery({ queryKey: ["dev-repo"], queryFn: ({ signal }) => devApi.repo(signal), enabled: on, refetchInterval: 10000 });
  const settings = useQuery({ queryKey: ["dev-settings"], queryFn: ({ signal }) => devApi.settings(signal), enabled: on });
  const [busy, setBusy] = useState(""); const [output, setOutput] = useState(""); const [diff, setDiff] = useState<DevDiff | null>(null);
  const [message, setMessage] = useState(""); const [confirmReset, setConfirmReset] = useState(false);
  const abort = useRef<AbortController | null>(null);
  if (status.isPending) return <Skeleton card />;
  if (!on) return <p className={styles.note}>{t.dev.off}</p>;
  async function act(name: string, action: () => Promise<void>) {
    setBusy(name);
    try { await action(); await client.invalidateQueries({ queryKey: ["dev-repo"] }); }
    catch (error) { notify(errorText(error, t.error)); } finally { setBusy(""); }
  }
  const stream = (kind: "test" | "release" | "rebuild" | "web-build") => act(kind, async () => {
    setOutput(""); abort.current = new AbortController();
    await devApi.run(kind, abort.current.signal, (event, data) => { const text = line(event, data); if (text) setOutput(current => `${current}${text}\n`); });
  });
  const r = repo.data;
  return <div className={styles.form}>
    <section className={styles.group} aria-label={t.dev.repo}><h3>{t.dev.repo}</h3>
      {repo.isPending && <Skeleton />}{repo.error && <p role="alert">{repo.error.message}</p>}
      {r && <dl className={styles.facts}><dt>{t.dev.path}</dt><dd>{r.path}</dd><dt>{t.dev.branch}</dt><dd>{r.branch || t.dev.detached}</dd>
        <dt>{t.dev.changed}</dt><dd>{r.changed_files}</dd><dt>{t.dev.lastCommit}</dt><dd>{r.last_commit.hash} · {r.last_commit.subject} · {r.last_commit.author}</dd></dl>}
      <div className={styles.actions}>
        <Button disabled={Boolean(busy)} onClick={() => void act("diff", async () => setDiff(await devApi.diff()))}>{t.dev.diff}</Button>
        <Button disabled={Boolean(busy)} onClick={() => void act("checkpoint", async () => {
          const result = await devApi.checkpoint(); notify(result.skipped ? result.message || t.dev.noChange : t.dev.checkpointDone(result.hash));
        })}>{t.dev.checkpoint}</Button>
        <Button variant="danger" disabled={Boolean(busy)} onClick={() => setConfirmReset(true)}>{t.dev.reset}</Button>
      </div>
      {confirmReset && <div className={styles.confirm} role="group"><p>{t.dev.confirmReset}</p>
        <Button variant="danger" onClick={() => { setConfirmReset(false); void act("reset", async () => { await devApi.reset(); setDiff(null); notify(t.dev.done); }); }}>{t.dev.reset}</Button>
        <Button onClick={() => setConfirmReset(false)}>{t.cancel}</Button></div>}
      <label>{t.dev.commitMessage}<span className={styles.inline}><input value={message} onChange={event => setMessage(event.target.value)} />
        <Button disabled={Boolean(busy) || !message.trim()} onClick={() => void act("commit", async () => {
          const result = await devApi.commit(message.trim()); setMessage(""); notify(result.hash ? `${t.dev.commit} ${result.hash}` : result.message || t.dev.done);
        })}>{t.dev.commit}</Button></span></label>
    </section>
    {diff && <section aria-label={t.dev.diff}><p className={styles.pre}>{diff.stat || t.dev.noDiff}</p>
      {diff.untracked.length > 0 && <p className={styles.pre}>{t.dev.untracked}:{"\n"}{diff.untracked.join("\n")}</p>}
      <pre className={styles.log}>{diff.diff.split("\n").map((text, index) => <span key={index}
        data-diff={text.startsWith("+") && !text.startsWith("+++") ? "add" : text.startsWith("-") && !text.startsWith("---") ? "remove" : undefined}>{text}{"\n"}</span>)}</pre>
      {diff.truncated && <p className={styles.note}>{t.dev.truncated}</p>}</section>}
    <section className={styles.group}><div className={styles.actions}>
      <Button disabled={Boolean(busy)} onClick={() => void stream("test")}>{t.dev.test}</Button>
      <Button disabled={Boolean(busy)} onClick={() => void stream("release")}>{t.dev.release}</Button>
      <Button disabled={Boolean(busy)} onClick={() => void stream("web-build")}>{t.dev.webBuild}</Button>
      <Button variant="primary" disabled={Boolean(busy)} onClick={() => void stream("rebuild")}>{t.dev.rebuild}</Button>
      {busy && <Button variant="danger" onClick={() => abort.current?.abort()}>{t.extensions.stop}</Button>}</div>
      <label className={styles.check}><input type="checkbox" checked={settings.data?.auto_rebuild ?? false} disabled={!settings.data}
        onChange={event => void devApi.saveSettings(event.target.checked).then(value => client.setQueryData(["dev-settings"], value)).catch(error => notify(errorText(error, t.error)))} />
        <span>{t.dev.autoRebuild}</span></label>
      {(busy || output) && <><h3>{t.dev.output}{busy ? ` · ${t.dev.running}` : ""}</h3><pre className={styles.log} aria-live="polite">{output}</pre></>}
    </section>
  </div>;
}
