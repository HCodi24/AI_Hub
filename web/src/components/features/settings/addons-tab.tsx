import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Plus, Trash2 } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { AddonChange, AddonHome, AddonKind } from "@/schema/addons.schema";
import { settingsText as t } from "@/text/settings";
import { AddonLink, CodexPlugins } from "./addon-link";
import { McpSection } from "./mcp-section";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
type Item = { name: string; detail: string };
const items = (home: AddonHome, kind: AddonKind): Item[] => kind === "plugin"
  ? home.plugins.map(plugin => ({ name: plugin.id, detail: [plugin.version, plugin.enabled ? "" : t.addons.disabled].filter(Boolean).join(" · ") }))
  : kind === "skill" ? home.skills.map(name => ({ name, detail: "" }))
    : home.mcp.map(server => ({ name: server.name, detail: server.summary }));
const KINDS: AddonKind[] = ["plugin", "skill", "mcp"];
const TABS = ["claude", "codex", "app"] as const;
type Tab = (typeof TABS)[number];
export function AddonsTab() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["addons"], queryFn: ({ signal }) => addonsApi.overview(signal) });
  const [busy, setBusy] = useState(""); const [output, setOutput] = useState(""); const [confirm, setConfirm] = useState(""); const [tab, setTab] = useState<Tab>("claude");
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
  function removeControl(change: AddonChange, label: string) {
    const id = key(change);
    if (busy === id) return <Loader2 size={16} className="spin" aria-label={t.addons.removing} />;
    if (confirm === id) return <>
      <Button size="sm" variant="danger" disabled={Boolean(busy)} onClick={() => void apply([change])}>{t.addons.remove}</Button>
      <Button size="sm" onClick={() => setConfirm("")}>{t.cancel}</Button>
    </>;
    return <Button size="sm" variant="ghost" disabled={Boolean(busy)} aria-label={label} onClick={() => setConfirm(id)}>
      <Trash2 size={16} aria-hidden="true" />{t.addons.remove}
    </Button>;
  }
  const missing = KINDS.flatMap(kind => items(data.personal, kind)
    .filter(item => !items(data.hub, kind).some(have => have.name === item.name))
    .map(item => ({ target: "claude", kind, name: item.name, action: "add" }) satisfies AddonChange));
  function onTabKey(event: React.KeyboardEvent<HTMLButtonElement>) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    event.preventDefault(); const next = TABS[(TABS.indexOf(tab) + step + TABS.length) % TABS.length];
    setTab(next); document.getElementById(`addons-tab-${next}`)?.focus();
  }
  return <div className={styles.form}>
    <AddonLink onOutput={setOutput} />

    <div className={styles.subTabs} role="tablist" aria-label={t.addons.tabsLabel}>
      {TABS.map(id => <button key={id} type="button" role="tab" id={`addons-tab-${id}`} aria-selected={tab === id} aria-controls={`addons-panel-${id}`}
        tabIndex={tab === id ? 0 : -1} onClick={() => setTab(id)} onKeyDown={onTabKey}>{t.addons.tabs[id]}</button>)}
    </div>
    <div role="tabpanel" id={`addons-panel-${tab}`} aria-labelledby={`addons-tab-${tab}`} className={styles.form}>
      {tab === "claude" && <section className={styles.addonInventory} aria-label={t.addons.claude}>
        {missing.length > 0 && <div className={styles.addonInventoryBanner}><span>{t.addons.banner(missing.length)}</span>
          <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply(missing)}><Plus size={16} aria-hidden="true" />{t.addons.addAll}</Button></div>}
        {KINDS.map(kind => {
          const hub = items(data.hub, kind);
          // What the user's own Claude Code has and MutiS does not yet: the source for "Thêm".
          const available = items(data.personal, kind).filter(item => !hub.some(have => have.name === item.name));
          const row = (item: Item, inHub: boolean) => {
            const add: AddonChange = { target: "claude", kind, name: item.name, action: "add" };
            const remove: AddonChange = { target: "claude", kind, name: item.name, action: "remove" };
            return <li key={item.name}>
              <span className={styles.rowText}><strong>{item.name}</strong>{item.detail && <small title={item.detail}>{item.detail}</small>}</span>
              {inHub ? removeControl(remove, t.addons.confirmRemove(item.name))
                : busy === key(add) ? <Loader2 size={16} className="spin" aria-label={t.addons.adding} />
                  : <Button size="sm" disabled={Boolean(busy)} onClick={() => void apply([add])}><Plus size={16} aria-hidden="true" />{t.addons.add}</Button>}
            </li>;
          };
          const empty = t.addons.emptyHub[kind];
          const availableText = t.addons.available(available.length);
          return <div key={kind} className={styles.addonInventoryGroup}>
            <h4>{t.addons.kinds[kind]}</h4>
            {hub.length > 0 && <ul className={styles.rows}>{hub.map(item => row(item, true))}</ul>}
            {available.length > 0 ? <details>
              <summary>{hub.length === 0 ? `${empty} · ${availableText}` : availableText}</summary>
              <ul className={styles.rows}>{available.map(item => row(item, false))}</ul>
            </details> : hub.length === 0 && <p className={styles.note}>{empty}</p>}
          </div>;
        })}
        <p className={styles.note}>{t.addons.pluginNote}</p>
      </section>}
      {tab === "codex" && <CodexPlugins onOutput={setOutput} />}
      {tab === "app" && <>
        <section className={styles.group} aria-label={t.addons.sharedSkills.title}>
          <h4>{t.addons.sharedSkills.title}</h4>
          {data.shared_skills.length === 0 ? <p className={styles.note}>{t.addons.sharedSkills.empty}</p> : <>
            <p className={styles.note}>{t.addons.sharedSkills.hint}</p>
            <ul className={styles.rows}>{data.shared_skills.map(name => <li key={name}>
              <span className={styles.rowText}><strong>/{name}</strong></span>
              {removeControl({ target: "mutis", kind: "skill", name, action: "remove" }, t.addons.sharedSkills.confirmRemove(name))}
            </li>)}</ul>
          </>}
        </section>
        <McpSection />
      </>}
    </div>
    {output && <details key={output} open><summary>{t.addons.output}</summary><pre className={styles.log} aria-live="polite">{output}</pre></details>}
  </div>;
}
