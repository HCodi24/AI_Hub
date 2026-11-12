import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Plus, Trash2 } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { AddonChange, AddonHome, AddonKind } from "@/schema/addons.schema";
import { settingsText as t } from "@/text/settings";
import { McpSection } from "./mcp-section";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
type Item = { name: string; detail: string };
const items = (home: AddonHome, kind: AddonKind): Item[] => kind === "plugin"
  ? home.plugins.map(plugin => ({ name: plugin.id, detail: [plugin.version, plugin.enabled ? "" : t.addons.disabled].filter(Boolean).join(" · ") }))
  : kind === "skill" ? home.skills.map(name => ({ name, detail: "" }))
    : home.mcp.map(server => ({ name: server.name, detail: server.summary }));
const KINDS: AddonKind[] = ["plugin", "skill", "mcp"];
/** Add-ons Claude Code and Codex load inside AI Hub, and copying the user's own Claude Code add-ons across. */
export function AddonsTab() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["addons"], queryFn: ({ signal }) => addonsApi.overview(signal) });
  const [busy, setBusy] = useState(""); const [output, setOutput] = useState(""); const [confirm, setConfirm] = useState("");
  if (query.isPending) return <Skeleton card />;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const data = query.data;
  const key = (change: AddonChange) => `${change.target}:${change.kind}:${change.name}`;
  async function apply(changes: AddonChange[]) {
    setConfirm("");
    for (const change of changes) {
      setBusy(key(change));
      try {
        const result = await addonsApi.change(change);
        if (result.output) setOutput(result.output);
        notify(change.action === "add" ? t.addons.added(change.name) : t.addons.removed(change.name));
      } catch (error) { const message = errorText(error, t.error); setOutput(message); notify(message); break; }
    }
    setBusy("");
    await client.invalidateQueries({ queryKey: ["addons"] });
  }
  const missing = KINDS.flatMap(kind => items(data.personal, kind)
    .filter(item => !items(data.hub, kind).some(have => have.name === item.name))
    .map(item => ({ target: "claude", kind, name: item.name, action: "add" }) satisfies AddonChange));
  return <div className={styles.form}>
    <section className={styles.group} aria-label={t.addons.claude}>
      <header className={styles.cardHead}><h3>{t.addons.claude}</h3>
        {missing.length > 0 && <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply(missing)}>
          <Plus size={16} aria-hidden="true" />{t.addons.addAll} ({missing.length})</Button>}</header>
      <p className={styles.note}>{t.addons.intro}</p>
      {KINDS.map(kind => {
        const mine = items(data.personal, kind); const hub = items(data.hub, kind);
        // Everything the user has, plus anything that exists only in AI Hub's Claude.
        const rows = [...mine.map(item => ({ ...item, inHub: hub.some(have => have.name === item.name), mine: true })),
          ...hub.filter(item => !mine.some(have => have.name === item.name)).map(item => ({ ...item, inHub: true, mine: false }))];
        return <div key={kind}><h4>{t.addons.kinds[kind]}</h4>
          {!rows.length && <p className={styles.note}>{t.addons.none}</p>}
          <ul className={styles.rows}>{rows.map(row => {
            const add: AddonChange = { target: "claude", kind, name: row.name, action: "add" };
            const remove: AddonChange = { target: "claude", kind, name: row.name, action: "remove" };
            const working = busy === key(add) || busy === key(remove);
            return <li key={row.name}>
              <span className={styles.rowText}><strong>{row.name}</strong>{row.detail && <small title={row.detail}>{row.detail}</small>}</span>
              <span className={styles.badge} data-on={row.inHub}>{row.inHub ? (row.mine ? t.addons.inHub : t.addons.onlyHub) : t.addons.notInHub}</span>
              {working ? <Loader2 size={16} className="spin" aria-label={t.addons.adding} />
                : confirm === key(remove) ? <><Button size="sm" variant="danger" onClick={() => void apply([remove])}>{t.addons.remove}</Button>
                  <Button size="sm" onClick={() => setConfirm("")}>{t.cancel}</Button></>
                  : row.inHub ? <Button size="sm" variant="ghost" disabled={Boolean(busy)} aria-label={t.addons.confirmRemove(row.name)}
                    onClick={() => setConfirm(key(remove))}><Trash2 size={16} aria-hidden="true" />{t.addons.remove}</Button>
                    : <Button size="sm" disabled={Boolean(busy)} onClick={() => void apply([add])}><Plus size={16} aria-hidden="true" />{t.addons.add}</Button>}
            </li>; })}</ul></div>;
      })}
      <p className={styles.note}>{t.addons.pluginNote}</p>
    </section>
    <section className={styles.group} aria-label={t.addons.codex}><h3>{t.addons.codex}</h3>
      <p className={styles.note}>{t.addons.codexIntro}</p>
      <h4>{t.addons.codexMcp}</h4>
      <ul className={styles.rows}>
        {data.codex.mcp.map(server => <li key={server.name}><span className={styles.rowText}><strong>{server.name}</strong>
          <small title={server.summary}>{server.summary}</small></span><span className={styles.badge} data-on="true">{t.addons.inHub}</span></li>)}
        {data.personal.mcp.filter(server => !data.codex.mcp.some(have => have.name === server.name)).map(server => {
          const add: AddonChange = { target: "codex", kind: "mcp", name: server.name, action: "add" };
          return <li key={server.name}><span className={styles.rowText}><strong>{server.name}</strong>
            <small title={server.summary}>{t.addons.from} · {server.summary}</small></span>
            {busy === key(add) ? <Loader2 size={16} className="spin" aria-label={t.addons.adding} />
              : <Button size="sm" disabled={Boolean(busy)} onClick={() => void apply([add])}><Plus size={16} aria-hidden="true" />{t.addons.addToCodex}</Button>}</li>; })}
      </ul>
      <h4>{t.addons.codexPlugins}</h4>
      <ul className={styles.rows}>{data.codex.plugins.map(plugin => <li key={plugin.id}>
        <span className={styles.rowText}><strong>{plugin.id}</strong></span>
        <span className={styles.badge} data-on={plugin.enabled}>{plugin.enabled ? t.addons.inHub : t.addons.disabled}</span></li>)}</ul>
    </section>
    <McpSection />
    {output && <section aria-label={t.addons.output}><h4>{t.addons.output}</h4><pre className={styles.log} aria-live="polite">{output}</pre></section>}
  </div>;
}
