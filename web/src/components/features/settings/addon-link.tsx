import { useEffect, useId, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Plus, Trash2 } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import type { LinkFound, LinkSkillConflict, LinkTarget } from "@/schema/addons.schema";
import { settingsText as t } from "@/text/settings";
import { AddonRow, splitId } from "./addon-row";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
const TARGETS: LinkTarget[] = ["claude", "codex"];
const toggle = <T extends string>(list: T[], value: T, set: (next: T[]) => void) => set(list.includes(value) ? list.filter(item => item !== value) : [...list, value]);
export function AddonLink({ onOutput }: { onOutput: (text: string) => void }) {
  const client = useQueryClient();
  const skillGroup = useId();
  const repositoryId = useId();
  const [link, setLink] = useState("");
  const [found, setFound] = useState<LinkFound | null>(null);
  const [targets, setTargets] = useState<LinkTarget[]>([]);
  const [plugins, setPlugins] = useState<string[]>([]);
  const [skillId, setSkillId] = useState("");
  const [replacement, setReplacement] = useState<LinkSkillConflict | null>(null);
  const [busy, setBusy] = useState<"inspect" | "plugins" | "skill" | null>(null);
  const pendingInspection = useRef<AbortController | null>(null);
  useEffect(() => () => pendingInspection.current?.abort(), []);

  const labels = t.addons.link;
  const markets = found ? TARGETS.filter(target => found[target]) : [];
  const skill = found?.skills.find(item => item.id === skillId);
  const conflict = replacement?.skill_id === skill?.id ? replacement : null;
  const installing = busy === "plugins" || busy === "skill";

  function clearResult() {
    setFound(null);
    setTargets([]);
    setPlugins([]);
    setSkillId("");
    setReplacement(null);
  }

  function editLink(value: string) {
    pendingInspection.current?.abort();
    pendingInspection.current = null;
    if (busy === "inspect") setBusy(null);
    setLink(value);
    clearResult();
  }

  async function inspect() {
    if (busy || !link.trim()) return;
    const controller = new AbortController();
    pendingInspection.current = controller;
    clearResult();
    setBusy("inspect");
    try {
      const result = await addonsApi.inspectLink(link.trim(), controller.signal);
      if (controller.signal.aborted) return;
      setFound(result);
      const available = TARGETS.filter(target => result[target]);
      setTargets(available);
      setPlugins([...new Set(available.flatMap(target => result[target]?.plugins.map(plugin => plugin.name) ?? []))]);
    } catch (error) {
      if (!controller.signal.aborted) notify(errorText(error, t.error));
    } finally {
      if (pendingInspection.current === controller) {
        pendingInspection.current = null;
        setBusy(null);
      }
    }
  }

  function report(text: string, message = text) {
    onOutput(text);
    notify(message);
  }

  async function installPlugins() {
    if (!found || busy) return;
    setBusy("plugins");
    try {
      const result = await addonsApi.installLink({ link: found.source, targets, plugins });
      report(result.output, result.ok ? t.addons.added(link.trim()) : result.output);
      if (result.ok && !found.skills.length) clearResult();
    } catch (error) {
      report(errorText(error, t.error));
    } finally {
      setBusy(null);
      await client.invalidateQueries({ queryKey: ["codex-plugins"] });
      await client.invalidateQueries({ queryKey: ["addons"] });
    }
  }

  async function installSkill(confirmation?: string) {
    if (!found || !skill || busy) return;
    setBusy("skill");
    setReplacement(null);
    try {
      const result = await addonsApi.installLinkSkill({ link: found.source, skill_id: skill.id, confirmation });
      report(result.output);
      if (result.conflict) setReplacement(result.conflict);
      if (result.ok) {
        setSkillId("");
        await client.invalidateQueries({ queryKey: ["addons"] });
      }
    } catch (error) {
      report(errorText(error, t.error));
    } finally {
      setBusy(null);
    }
  }

  const chosen = new Set(targets.flatMap(target => found?.[target]?.plugins.map(plugin => plugin.name) ?? []));
  const pluginsReady = targets.length > 0 && plugins.some(name => chosen.has(name));
  return <section className={styles.group} aria-label={labels.title}>
    <h3>{labels.title}</h3>
    <p className={styles.note}>{labels.intro}</p>
    <form className={styles.repoForm} onSubmit={event => { event.preventDefault(); void inspect(); }}>
      <label htmlFor={repositoryId}>{labels.repository}</label>
      <div className={styles.repoFormRow}>
        <input
          id={repositoryId}
          value={link}
          onChange={event => editLink(event.target.value)}
          placeholder={labels.placeholder}
          disabled={installing}
          required
        />
        <Button type="submit" variant="primary" disabled={Boolean(busy) || !link.trim()}>
          {busy === "inspect" ? labels.checking : labels.check}
        </Button>
      </div>
    </form>
    {found && !markets.length && !found.skills.length && <p className={styles.note} role="status">{labels.none}</p>}
    {found && markets.length > 0 && <div className={styles.form}>
      <h4>{labels.pickPlugins}</h4>
      <div className={styles.actions} role="group" aria-label={labels.pickTarget}>
        {markets.map(target => <label key={target} className={styles.check}>
          <input type="checkbox" checked={targets.includes(target)} disabled={installing} onChange={() => toggle(targets, target, setTargets)} />
          {t.extensions[target]}
        </label>)}
      </div>
      <ul className={styles.rows}>
        {[...chosen].map(name => {
          const description = markets.map(target => found[target]?.plugins.find(plugin => plugin.name === name)?.description).find(Boolean);
          return <li key={name}><label className={styles.check}>
            <input type="checkbox" checked={plugins.includes(name)} disabled={installing} onChange={() => toggle(plugins, name, setPlugins)} />
            <strong>{name}</strong>{description && <small title={description}>{description}</small>}
          </label></li>;
        })}
      </ul>
      <p className={styles.note}>{labels.warn}</p>
      <Button variant="primary" disabled={Boolean(busy) || !pluginsReady} onClick={() => void installPlugins()}>
        {busy === "plugins" ? labels.installing : labels.installPlugins}
      </Button>
    </div>}
    {found && found.skills.length > 0 && <div className={styles.form}>
      <h4>{labels.pickSkills}</h4>
      <div role="radiogroup" aria-label={labels.skillChoice}>
        <ul className={styles.rows}>
          {found.skills.map(item => <li key={item.id}><label className={styles.check}>
            <input type="radio" name={skillGroup} checked={skillId === item.id} disabled={installing}
              onChange={() => { setSkillId(item.id); setReplacement(null); }} />
            <strong>{item.name}</strong><small>{item.path}</small>
          </label></li>)}
        </ul>
      </div>
      {skill && conflict ? <div className={styles.confirm} role="group" aria-label={labels.replaceSkill}>
        <p>{labels.confirmReplace(skill.name)}</p>
        <Button variant="danger" disabled={installing} onClick={() => void installSkill(conflict.confirmation)}>{labels.replaceSkill}</Button>
        <Button disabled={installing} onClick={() => setReplacement(null)}>{t.cancel}</Button>
      </div> : <Button variant="primary" disabled={Boolean(busy) || !skill} onClick={() => void installSkill()}>
        {busy === "skill" ? labels.installing : labels.installSkill}
      </Button>}
    </div>}
  </section>;
}
/** Plugins in MutiS's own Codex, plus the ones only the user's own Codex has (copied over with "Thêm"). */
export function CodexPlugins({ onOutput, matches = () => true, search = "" }: { onOutput: (text: string) => void; matches?: (text: string) => boolean; search?: string }) {
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
  const hub = codex.data?.plugins ?? [];
  const available = codex.data?.available ?? [];
  const empty = t.addons.emptyHub.plugin;
  const shownHub = hub.filter(plugin => matches(plugin.id));
  const shownAvailable = available.filter(plugin => matches(plugin.id));
  const availableText = t.addons.availableCodex(shownAvailable.length);
  const noMatch = search && shownHub.length === 0 && shownAvailable.length === 0 && hub.length + available.length > 0;
  const row = (plugin: { id: string; version: string }, inHub: boolean) => <AddonRow key={plugin.id} kind="plugin" {...splitId(plugin.id)} detail={plugin.version}>
    {busy === plugin.id ? <Loader2 size={16} className="spin" aria-label={inHub ? t.addons.removing : t.addons.adding} />
      : confirm === plugin.id ? <><Button size="sm" variant="danger" onClick={() => void apply([plugin.id], "remove")}>{t.addons.remove}</Button><Button size="sm" onClick={() => setConfirm("")}>{t.cancel}</Button></>
        : inHub ? <Button size="sm" variant="ghost" disabled={Boolean(busy)} aria-label={t.addons.confirmRemove(plugin.id)} onClick={() => setConfirm(plugin.id)}><Trash2 size={16} aria-hidden="true" />{t.addons.remove}</Button>
          : <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply([plugin.id], "add")}><Plus size={16} aria-hidden="true" />{t.addons.add}</Button>}
  </AddonRow>;
  return <section className={styles.addonInventory} aria-label={t.addons.link.codexTitle}>
    {available.length > 0 && <div className={styles.addonInventoryBanner}><span>{t.addons.bannerCodex(available.length)}</span>
      <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply(available.map(plugin => plugin.id), "add")}><Plus size={16} aria-hidden="true" />{t.addons.addAll}</Button></div>}
    <div className={styles.addonInventoryGroup}>
      {noMatch ? <p className={styles.note}>{t.addons.noMatch(search)}</p> : <>
        {shownHub.length > 0 && <ul className={styles.addonCard}>{shownHub.map(plugin => row(plugin, true))}</ul>}
        {shownAvailable.length > 0 ? <>
          <h4>{shownHub.length === 0 ? `${empty} · ${availableText}` : availableText}</h4>
          <ul className={styles.addonCard}>{shownAvailable.map(plugin => row(plugin, false))}</ul>
        </> : hub.length === 0 && <p className={styles.note}>{empty}</p>}
      </>}
    </div>
  </section>;
}
