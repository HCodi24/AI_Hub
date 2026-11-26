import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, Loader2, Plus, Search, Trash2 } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { Button } from "@/components/button/button";
import { Switch } from "@/components/button/switch";
import { PillTabs } from "@/components/pill-tabs/pill-tabs";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { AddonChange, AddonHome, AddonKind } from "@/schema/addons.schema";
import { settingsText as t } from "@/text/settings";
import { AddonLink, CodexPlugins } from "./addon-link";
import { AddonRow, splitId } from "./addon-row";
import { McpSection } from "./mcp-section";
import { errorText, useSettingsConfig } from "../use-settings";
import styles from "../settings.module.css";
type Item = { name: string; detail: string; enabled?: boolean };
const items = (home: AddonHome, kind: AddonKind): Item[] => kind === "plugin"
  ? home.plugins.map(plugin => ({ name: plugin.id, detail: plugin.version, enabled: plugin.enabled }))
  : kind === "skill" ? home.skills.map(name => ({ name, detail: "" }))
    : home.mcp.map(server => ({ name: server.name, detail: server.summary, enabled: server.enabled }));
const KINDS: AddonKind[] = ["plugin", "skill", "mcp"];
const SOURCES = ["claude", "codex", "antigravity", "app"] as const;
type Source = (typeof SOURCES)[number];
/** Where each kind actually lives: Codex in MutiS only takes plugins; MutiS itself has shared skills and app MCPs, no plugins.
 * All three source tabs stay visible; the pairs missing here show a short explanation instead of a list. */
const SOURCES_FOR: Record<AddonKind, Source[]> = { plugin: ["claude", "codex", "antigravity"], mcp: ["claude", "codex", "app"], skill: ["claude", "app"] };
const PANEL = "addons-panel";
/** "Thêm" dropdown: jumps to the kind/source pair where that add-on is added. */
const MENU: Record<Source, { kind?: AddonKind; source: Source }> = { claude: { source: "claude" }, codex: { kind: "plugin", source: "codex" }, antigravity: { kind: "plugin", source: "antigravity" }, app: { kind: "mcp", source: "app" } };
function AddMenu({ onPick }: { onPick: (source: Source) => void }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { setOpen(false); root.current?.querySelector<HTMLButtonElement>("button")?.focus(); } };
    document.addEventListener("pointerdown", outside); document.addEventListener("keydown", escape);
    root.current?.querySelector<HTMLButtonElement>("[role=menuitem]")?.focus();
    return () => { document.removeEventListener("pointerdown", outside); document.removeEventListener("keydown", escape); };
  }, [open]);
  function onMenuKey(event: React.KeyboardEvent<HTMLDivElement>) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const items = [...(root.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]") ?? [])];
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    items[(at + step + items.length) % items.length]?.focus();
  }
  return <div className={styles.addMenu} ref={root}>
    <Button size="sm" variant="primary" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(value => !value)}>
      {t.addons.head.add}<ChevronDown size={16} aria-hidden="true" />
    </Button>
    {open && <div role="menu" aria-label={t.addons.head.menu} className={styles.addMenuList} onKeyDown={onMenuKey}>
      {SOURCES.map(id => <button key={id} type="button" role="menuitem" tabIndex={-1} onClick={() => { setOpen(false); onPick(id); }}>{t.addons.head.items[id]}</button>)}
    </div>}
  </div>;
}
export function AddonsTab() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["addons"], queryFn: ({ signal }) => addonsApi.overview(signal) });
  // Same cache keys as CodexPlugins and McpSection, so the tab counts stay in sync with what those panels show.
  const codex = useQuery({ queryKey: ["codex-plugins"], queryFn: ({ signal }) => addonsApi.codexPlugins(signal) });
  const agy = useQuery({ queryKey: ["antigravity-plugins"], queryFn: ({ signal }) => addonsApi.antigravityPlugins(signal) });
  const { query: config } = useSettingsConfig();
  const [busy, setBusy] = useState(""); const [output, setOutput] = useState(""); const [confirm, setConfirm] = useState(""); const [showLink, setShowLink] = useState(false);
  const [kind, setKindState] = useState<AddonKind>("plugin"); const [source, setSource] = useState<Source>("claude");
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const matches = (text: string) => !needle || text.toLowerCase().includes(needle);
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
        notify(change.action === "add" ? t.addons.added(change.name) : change.action === "enable" ? t.addons.enabled(change.name)
          : change.action === "disable" ? t.addons.disabledNow(change.name) : t.addons.removed(change.name));
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
      <Trash2 size={16} aria-hidden="true" />
    </Button>;
  }
  const setKind = setKindState;
  function pick(next: Source) {
    const target = MENU[next];
    if (target.kind) setKindState(target.kind);
    setSource(target.source);
    document.getElementById(`addons-kind-tab-${target.kind ?? kind}`)?.focus();
  }
  const count = (k: AddonKind, s: Source): number | undefined => !SOURCES_FOR[k].includes(s) ? undefined
    : s === "claude" ? items(data.hub, k).length
    : s === "codex" ? (k === "mcp" ? codex.data?.mcp.length : codex.data?.plugins.length)
    : s === "antigravity" ? (k === "plugin" ? agy.data?.plugins.length : undefined)
      : k === "skill" ? data.shared_skills.length : config.data?.mcp_servers.length;
  const kindCount = (k: AddonKind) => SOURCES_FOR[k].reduce((sum, s) => sum + (count(k, s) ?? 0), 0);
  const active = source;
  const supported = SOURCES_FOR[kind].includes(active);

  function claudePanel() {
    const hub = items(data.hub, kind);
    // What the user's own Claude Code has and MutiS does not yet: the source for "Thêm".
    const available = items(data.personal, kind).filter(item => !hub.some(have => have.name === item.name));
    const missing = available.map(item => ({ target: "claude", kind, name: item.name, action: "add" }) satisfies AddonChange);
    const row = (item: Item, inHub: boolean) => {
      const add: AddonChange = { target: "claude", kind, name: item.name, action: "add" };
      const remove: AddonChange = { target: "claude", kind, name: item.name, action: "remove" };
      const { name, tag } = kind === "plugin" ? splitId(item.name) : { name: item.name, tag: "" };
      // MCP servers run commands on this machine, so each one is off until switched on here (plugins load as soon as they are installed).
      const toggle: AddonChange | null = inHub && kind === "mcp" ? { target: "claude", kind: "mcp", name: item.name, action: item.enabled ? "disable" : "enable" } : null;
      return <AddonRow key={item.name} kind={kind} name={name} tag={tag} detail={item.detail} muted={Boolean(toggle) && !item.enabled}>
        {inHub ? removeControl(remove, t.addons.confirmRemove(item.name))
          : busy === key(add) ? <Loader2 size={16} className="spin" aria-label={t.addons.adding} />
            : <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply([add])}><Plus size={16} aria-hidden="true" />{t.addons.add}</Button>}
        {toggle && (busy === key(toggle) ? <Loader2 size={16} className="spin" aria-label={t.addons.toggling} />
          : <Switch checked={Boolean(item.enabled)} disabled={Boolean(busy)} aria-label={t.addons.toggle(item.name)} onChange={() => void apply([toggle])} />)}
      </AddonRow>;
    };
    const empty = t.addons.emptyHub[kind];
    const shownHub = hub.filter(item => matches(item.name));
    const shownAvailable = available.filter(item => matches(item.name));
    const availableText = t.addons.available(shownAvailable.length);
    const noMatch = needle && shownHub.length === 0 && shownAvailable.length === 0 && hub.length + available.length > 0;
    return <section className={styles.addonInventory} aria-label={t.addons.claude}>
      {missing.length > 0 && <div className={styles.addonInventoryBanner}><span>{t.addons.banner(missing.length)}</span>
        <Button size="sm" variant="primary" disabled={Boolean(busy)} onClick={() => void apply(missing)}><Plus size={16} aria-hidden="true" />{t.addons.addAll}</Button></div>}
      <div className={styles.addonInventoryGroup}>
        {noMatch ? <p className={styles.note}>{t.addons.noMatch(search.trim())}</p> : <>
          {shownHub.length > 0 && <ul className={styles.addonCard}>{shownHub.map(item => row(item, true))}</ul>}
          {shownAvailable.length > 0 ? <>
            <h4>{shownHub.length === 0 ? `${empty} · ${availableText}` : availableText}</h4>
            <ul className={styles.addonCard}>{shownAvailable.map(item => row(item, false))}</ul>
          </> : hub.length === 0 && <p className={styles.note}>{empty}</p>}
        </>}
      </div>
      {kind === "plugin" && <p className={styles.note}>{t.addons.pluginNote}</p>}
    </section>;
  }

  return <div className={styles.form}>
    <header className={styles.addonHead}>
      <div className={styles.addonHeadText}>
        <h3 className={styles.panelTitle}>{t.tabs.addons}</h3>
        <p>{t.addons.head.subtitle}</p>
      </div>
      <div className={styles.addonHeadActions}>
        <Button size="sm" aria-expanded={showLink} aria-controls="addons-link" onClick={() => setShowLink(open => !open)}>{t.addons.head.github}</Button>
        <AddMenu onPick={pick} />
      </div>
    </header>
    {showLink && <div id="addons-link"><AddonLink onOutput={setOutput} /></div>}
    <div className={styles.addonTabs}>
      <PillTabs idPrefix="addons-kind" controls={PANEL} label={t.addons.kindTabsLabel} value={kind} onChange={setKind}
        tabs={KINDS.map(id => ({ id, label: t.addons.kindTabs[id], count: kindCount(id) }))} />
      <PillTabs idPrefix="addons-source" controls={PANEL} label={t.addons.tabsLabel} value={active} onChange={setSource}
        tabs={SOURCES.map(id => ({ id, label: t.addons.tabs[id], count: count(kind, id) }))} />
    </div>
    <label className={styles.addonSearch}>
      <Search size={16} aria-hidden="true" />
      <input type="search" aria-label={t.addons.search[kind]} placeholder={t.addons.searchPlaceholder} value={search}
        onChange={event => setSearch(event.target.value)} onKeyDown={event => { if (event.key === "Escape" && search) { event.stopPropagation(); setSearch(""); } }} />
    </label>
    <div role="tabpanel" id={PANEL} aria-labelledby={`addons-kind-tab-${kind} addons-source-tab-${active}`} className={styles.form}>
      {!supported && <div className={styles.addonInventoryBanner}>
        <span>{active === "app" ? t.addons.unsupported.appPlugin : t.addons.unsupported[active === "antigravity" ? "antigravity" : "codex"][kind === "skill" ? "skill" : "mcp"]}</span>
        {active !== "app" && active !== "claude" && <Button size="sm" onClick={() => setSource("app")}>{t.addons.unsupported.goApp}</Button>}
      </div>}
      {active === "claude" && claudePanel()}
      {active === "codex" && kind === "mcp" && <section className={styles.addonInventory} aria-label={t.addons.tabs.codex}>
        {(codex.data?.mcp ?? []).filter(server => matches(server.name)).length === 0 ? <p className={styles.note}>{t.addons.emptyCodexMcp}</p>
          : <ul className={styles.addonCard}>{(codex.data?.mcp ?? []).filter(server => matches(server.name)).map(server => {
            const toggle: AddonChange = { target: "codex", kind: "mcp", name: server.name, action: server.enabled ? "disable" : "enable" };
            return <AddonRow key={server.name} kind="mcp" name={server.name} tag="" detail={server.summary} muted={!server.enabled}>
              {busy === key(toggle) ? <Loader2 size={16} className="spin" aria-label={t.addons.toggling} />
                : <Switch checked={server.enabled} disabled={Boolean(busy)} aria-label={t.addons.toggle(server.name)}
                  onChange={() => void apply([toggle]).then(() => client.invalidateQueries({ queryKey: ["codex-plugins"] }))} />}
            </AddonRow>; })}</ul>}
      </section>}
      {active === "codex" && kind === "plugin" && <CodexPlugins onOutput={setOutput} matches={matches} search={search.trim()} />}
      {active === "antigravity" && kind === "plugin" && <CodexPlugins target="antigravity" onOutput={setOutput} matches={matches} search={search.trim()} />}
      {active === "app" && kind === "skill" && <section className={styles.addonInventory} aria-label={t.addons.sharedSkills.title}>
        {data.shared_skills.length === 0 ? <p className={styles.note}>{t.addons.sharedSkills.empty}</p>
          : !data.shared_skills.some(matches) ? <p className={styles.note}>{t.addons.noMatch(search.trim())}</p>
          : <ul className={styles.addonCard}>{data.shared_skills.filter(matches).map(name => <AddonRow key={name} kind="skill" name={`/${name}`}>
            {removeControl({ target: "mutis", kind: "skill", name, action: "remove" }, t.addons.sharedSkills.confirmRemove(name))}
          </AddonRow>)}</ul>}
      </section>}
      {active === "app" && kind === "mcp" && <McpSection matches={matches} />}
    </div>
    {output && <details key={output} open><summary>{t.addons.output}</summary><pre className={styles.log} aria-live="polite">{output}</pre></details>}
  </div>;
}
