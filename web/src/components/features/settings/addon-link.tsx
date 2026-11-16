import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Plus, Trash2 } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import type { LinkFound, LinkTarget } from "@/schema/addons.schema";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
const TARGETS: LinkTarget[] = ["claude", "codex"];
const toggle = <T extends string>(list: T[], value: T, set: (next: T[]) => void) => set(list.includes(value) ? list.filter(item => item !== value) : [...list, value]);
/** Paste a GitHub link: shows which of Claude / Codex it supports, installs the chosen plugins into MutiS's own copies. */
export function AddonLink({ onOutput }: { onOutput: (text: string) => void }) {
  const client = useQueryClient();
  const [link, setLink] = useState(""); const [found, setFound] = useState<LinkFound | null>(null);
  const [targets, setTargets] = useState<LinkTarget[]>([]); const [plugins, setPlugins] = useState<string[]>([]);
  const [busy, setBusy] = useState("");
  const markets = found ? TARGETS.filter(target => found[target]) : [];
  async function inspect() {
    setBusy("inspect"); setFound(null);
    try {
      const result = await addonsApi.inspectLink(link.trim());
      setFound(result);
      const available = TARGETS.filter(target => result[target]);
      setTargets(available);
      setPlugins([...new Set(available.flatMap(target => result[target]?.plugins.map(plugin => plugin.name) ?? []))]);
    } catch (error) { notify(errorText(error, t.error)); }
    finally { setBusy(""); }
  }
  async function install() {
    setBusy("install");
    try {
      const result = await addonsApi.installLink({ link: link.trim(), targets, plugins });
      onOutput(result.output); notify(result.ok ? t.addons.added(link.trim()) : result.output);
      if (result.ok) setFound(null);
    } catch (error) { const message = errorText(error, t.error); onOutput(message); notify(message); }
    finally { setBusy(""); await client.invalidateQueries({ queryKey: ["codex-plugins"] }); await client.invalidateQueries({ queryKey: ["addons"] }); }
  }
  const chosen = new Set(targets.flatMap(target => found?.[target]?.plugins.map(plugin => plugin.name) ?? []));
  const ready = targets.length > 0 && plugins.some(name => chosen.has(name));
  return <section className={styles.group} aria-label={t.addons.link.title}><h3>{t.addons.link.title}</h3>
    <p className={styles.note}>{t.addons.link.intro}</p>
    <form className={styles.actions} onSubmit={event => { event.preventDefault(); void inspect(); }}>
      <input value={link} onChange={event => setLink(event.target.value)} placeholder={t.addons.link.placeholder} aria-label={t.addons.link.title} required />
      <Button type="submit" variant="primary" disabled={Boolean(busy) || !link.trim()}>{busy === "inspect" ? t.addons.link.checking : t.addons.link.check}</Button>
    </form>
    {found && (markets.length === 0 ? <p className={styles.note} role="status">{t.addons.link.none}</p> : <div className={styles.form}>
      <h4>{t.addons.link.pickTarget}</h4>
      <div className={styles.actions}>{markets.map(target => <label key={target} className={styles.check}>
        <input type="checkbox" checked={targets.includes(target)} onChange={() => toggle(targets, target, setTargets)} /> {t.extensions[target]}</label>)}</div>
      <h4>{t.addons.link.pickPlugins}</h4>
      <ul className={styles.rows}>{[...chosen].map(name => {
        const description = markets.map(target => found[target]?.plugins.find(plugin => plugin.name === name)?.description).find(Boolean) ?? "";
        return <li key={name}><label className={styles.check}><input type="checkbox" checked={plugins.includes(name)} onChange={() => toggle(plugins, name, setPlugins)} />
          <strong>{name}</strong>{description && <small title={description}>{description}</small>}</label></li>; })}</ul>
      <p className={styles.note}>{t.addons.link.warn}</p>
      <Button variant="primary" disabled={Boolean(busy) || !ready} onClick={() => void install()}>{busy === "install" ? t.addons.link.installing : t.addons.link.install}</Button>
    </div>)}
  </section>;
}
/** Plugins in MutiS's own Codex, plus the ones only the user's own Codex has (copied over with "Thêm"). */
export function CodexPlugins({ onOutput }: { onOutput: (text: string) => void }) {
  const client = useQueryClient();
  const codex = useQuery({ queryKey: ["codex-plugins"], queryFn: ({ signal }) => addonsApi.codexPlugins(signal) });
  const [busy, setBusy] = useState(""); const [confirm, setConfirm] = useState("");
  async function apply(ids: string[], action: "add" | "remove") {
    setConfirm("");
    for (const id of ids) {
      setBusy(id);
      try { const result = await addonsApi.change({ target: "codex", kind: "plugin", name: id, action }); onOutput(result.output); notify(action === "add" ? t.addons.added(id) : t.addons.removed(id)); }
      catch (error) { const message = errorText(error, t.error); onOutput(message); notify(message); break; }
    }
    setBusy(""); await client.invalidateQueries({ queryKey: ["codex-plugins"] });
  }
  const available = codex.data?.available ?? [];
  const row = (plugin: { id: string; version: string }, inHub: boolean) => <li key={plugin.id}>
    <span className={styles.rowText}><strong>{plugin.id}</strong>{plugin.version && <small>{plugin.version}</small>}</span>
    {busy === plugin.id ? <Loader2 size={16} className="spin" aria-label={inHub ? t.addons.removing : t.addons.adding} />
      : confirm === plugin.id ? <><Button size="sm" variant="danger" onClick={() => void apply([plugin.id], "remove")}>{t.addons.remove}</Button><Button size="sm" onClick={() => setConfirm("")}>{t.cancel}</Button></>
        : inHub ? <Button size="sm" variant="ghost" disabled={Boolean(busy)} aria-label={t.addons.confirmRemove(plugin.id)} onClick={() => setConfirm(plugin.id)}><Trash2 size={16} aria-hidden="true" />{t.addons.remove}</Button>
          : <Button size="sm" disabled={Boolean(busy)} onClick={() => void apply([plugin.id], "add")}><Plus size={16} aria-hidden="true" />{t.addons.add}</Button>}
  </li>;
  return <section className={styles.group} aria-label={t.addons.link.codexTitle}>
    {available.length > 0 && <div className={styles.banner}><span>{t.addons.bannerCodex(available.length)}</span>
      <Button size="sm" disabled={Boolean(busy)} onClick={() => void apply(available.map(plugin => plugin.id), "add")}><Plus size={16} aria-hidden="true" />{t.addons.addAll}</Button></div>}
    <h4>{t.addons.link.codexTitle}</h4>
    {!codex.data?.plugins.length && <p className={styles.note}>{t.addons.link.codexNone}</p>}
    <ul className={styles.rows}>{codex.data?.plugins.map(plugin => row(plugin, true))}</ul>
    {available.length > 0 && <details><summary>{t.addons.availableCodex(available.length)}</summary><ul className={styles.rows}>{available.map(plugin => row(plugin, false))}</ul></details>}
  </section>;
}
