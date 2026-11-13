import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { useProviderConnections } from "@/lib/provider-connections";
import type { ConnectionTest, FullConfig, Provider } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { ClaudeConnection } from "./claude-connection";
import { CodexConnection } from "./codex-connection";
import { ProviderForm } from "./provider-form";
import { useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";

const SUB_TABS = ["cli", "login", "api"] as const;
type SubTab = (typeof SUB_TABS)[number];
const CLI_KINDS = ["codex_cli", "claude_cli"] as const;
const API_KINDS = ["openai_compat", "anthropic"] as const;
const isCliKind = (kind: string) => kind === "codex_cli" || kind === "claude_cli";
/** Provider id per subtab; "" = new draft, null = nothing picked yet. */
type Selection = Record<SubTab, string | null>;
const NO_SELECTION: Selection = { cli: null, login: null, api: null };

function SourceList({ items, selected, statuses, pending, onSelect, onAdd, children }: {
  items: readonly Provider[]; selected: string | null; statuses: Record<string, ConnectionTest | undefined>;
  pending: boolean; onSelect: (id: string) => void; onAdd?: () => void; children?: ReactNode;
}) {
  return <nav className={styles.list} aria-label={t.providers.list}>
    {pending && <Skeleton />}
    {items.map(provider => { const status = statuses[provider.id];
      return <button key={provider.id} aria-current={provider.id === selected ? "true" : undefined} onClick={() => onSelect(provider.id)}>
        <span className={styles.dot} data-state={status ? (status.ok ? "ok" : "error") : "unknown"} aria-hidden="true" />
        <span><strong>{provider.name}</strong><small>{status?.message || t.providers.notTested}</small></span></button>; })}
    {onAdd && <Button onClick={onAdd}><Plus size={16} aria-hidden="true" />{t.providers.add}</Button>}
    {children}
  </nav>;
}

export function ProvidersTab() {
  const client = useQueryClient();
  const { query, apply } = useSettingsConfig();
  const cli = useQuery({ queryKey: ["cli-status"], queryFn: ({ signal }) => settingsApi.cliStatus(signal) });
  const [tab, setTab] = useState<SubTab>("cli");
  const [visited, setVisited] = useState<Record<SubTab, boolean>>({ cli: true, login: false, api: false });
  const [selection, setSelection] = useState<Selection>(NO_SELECTION);
  const tabRefs = useRef<Record<SubTab, HTMLButtonElement | null>>({ cli: null, login: null, api: null });
  const providers = query.data?.providers || [];
  const statuses = useProviderConnections(providers);
  const cliProviders = useMemo(() => providers.filter(provider => isCliKind(provider.kind)), [providers]);
  const apiProviders = useMemo(() => providers.filter(provider => !isCliKind(provider.kind)), [providers]);
  const lists = useMemo<Record<SubTab, readonly Provider[]>>(
    () => ({ cli: cliProviders, login: cliProviders, api: apiProviders }), [cliProviders, apiProviders]);

  // Keep every category's selection valid after load, save and delete.
  useEffect(() => {
    setSelection(current => {
      let next = current;
      for (const id of SUB_TABS) {
        const value = current[id];
        if (value === "") continue; // open draft stays open
        if (value && lists[id].some(provider => provider.id === value)) continue;
        const fallback = lists[id][0]?.id ?? null;
        if (value === fallback) continue;
        if (next === current) next = { ...current };
        next[id] = fallback;
      }
      return next;
    });
  }, [lists]);

  function open(id: SubTab) {
    setTab(id);
    setVisited(current => (current[id] ? current : { ...current, [id]: true }));
  }
  function select(id: SubTab, value: string | null) {
    setSelection(current => ({ ...current, [id]: value }));
  }
  function onTabKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    const index = SUB_TABS.indexOf(tab);
    let next = -1;
    if (event.key === "ArrowRight") next = (index + 1) % SUB_TABS.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + SUB_TABS.length) % SUB_TABS.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = SUB_TABS.length - 1;
    if (next < 0) return;
    event.preventDefault();
    const target = SUB_TABS[next];
    if (!target) return;
    open(target);
    tabRefs.current[target]?.focus();
  }

  const onSaved = (id: SubTab) => (config: FullConfig, providerId: string) => { apply(config); select(id, providerId); };
  const onDeleted = (id: SubTab) => (config: FullConfig) => {
    apply(config);
    const remaining = config.providers.filter(provider => (id === "api" ? !isCliKind(provider.kind) : isCliKind(provider.kind)));
    select(id, remaining[0]?.id ?? null);
  };
  const onTested = (providerId: string, result: ConnectionTest) => client.setQueryData(["provider-test", providerId], result);

  function form(id: SubTab, allowedKinds: readonly string[], defaultKind: string, showConnections: boolean) {
    const list = lists[id];
    const value = selection[id];
    const current = value ? list.find(provider => provider.id === value) ?? null : null;
    if (value === null) return <p className={styles.note}>{t.providers.selectHint}</p>;
    if (value !== "" && !current) return <p className={styles.note}>{t.providers.selectHint}</p>;
    return <ProviderForm key={`${id}:${value || "new"}`} provider={current}
      status={current ? statuses[current.id] : undefined}
      allowedKinds={allowedKinds} defaultKind={defaultKind} showConnections={showConnections}
      onSaved={onSaved(id)} onDeleted={onDeleted(id)} onTested={onTested} />;
  }

  const loginProvider = selection.login ? cliProviders.find(provider => provider.id === selection.login) ?? null : null;

  return <div className={styles.providers}>
    <div className={styles.subTabs} role="tablist" aria-label={t.providers.subTabsLabel}>
      {SUB_TABS.map(id => <button key={id} type="button" role="tab" id={`providers-subtab-${id}`}
        aria-selected={tab === id} aria-controls={`providers-panel-${id}`} tabIndex={tab === id ? 0 : -1}
        ref={node => { tabRefs.current[id] = node; }} onClick={() => open(id)} onKeyDown={onTabKeyDown}>
        {t.providers.subTabs[id]}</button>)}
    </div>
    <div className={styles.subPanels}>
      {visited.cli && <div className={styles.subPanel} role="tabpanel" id="providers-panel-cli"
        aria-labelledby="providers-subtab-cli" hidden={tab !== "cli"} tabIndex={0}>
        <div className={styles.split}>
          <SourceList items={cliProviders} selected={selection.cli} statuses={statuses} pending={query.isPending}
            onSelect={id => select("cli", id)} onAdd={() => select("cli", "")}>
            <section className={styles.cli} aria-label={t.providers.cli}><h4>{t.providers.cli}</h4>
              {(["codex_cli", "claude_cli"] as const).map(key => { const item = cli.data?.[key];
                return <p key={key}><strong>{t.providers.kinds[key]}</strong><br />
                  {item ? (item.installed ? item.version || t.providers.cliNoVersion : t.providers.cliMissing) : t.loading}
                  {item?.path && <small title={item.path}>{item.path}</small>}</p>; })}
            </section>
          </SourceList>
          <section className={styles.detail}>
            {query.error && <p role="alert">{query.error.message}</p>}
            <p className={styles.note}>{t.providers.cliHint}</p>
            {form("cli", CLI_KINDS, "codex_cli", false)}
          </section>
        </div>
      </div>}
      {visited.login && <div className={styles.subPanel} role="tabpanel" id="providers-panel-login"
        aria-labelledby="providers-subtab-login" hidden={tab !== "login"} tabIndex={0}>
        <div className={styles.split}>
          <SourceList items={cliProviders} selected={selection.login} statuses={statuses} pending={query.isPending}
            onSelect={id => select("login", id)}>
            {!query.isPending && !cliProviders.length && <p className={styles.note}>{t.providers.loginEmpty}</p>}
            {!query.isPending && !cliProviders.length && <Button onClick={() => open("cli")}>{t.providers.loginEmptyAction}</Button>}
          </SourceList>
          <section className={styles.detail}>
            {query.error && <p role="alert">{query.error.message}</p>}
            <p className={styles.note}>{t.providers.loginNote}</p>
            {loginProvider ? <>
              <p className={styles.loginHead}><strong>{loginProvider.name}</strong> · {t.providers.kinds[loginProvider.kind as keyof typeof t.providers.kinds] || loginProvider.kind}</p>
              {loginProvider.kind === "codex_cli" && <CodexConnection key={loginProvider.id} providerId={loginProvider.id} />}
              {loginProvider.kind === "claude_cli" && <ClaudeConnection key={loginProvider.id} providerId={loginProvider.id} />}
            </> : <p className={styles.note}>{t.providers.loginSelectHint}</p>}
          </section>
        </div>
      </div>}
      {visited.api && <div className={styles.subPanel} role="tabpanel" id="providers-panel-api"
        aria-labelledby="providers-subtab-api" hidden={tab !== "api"} tabIndex={0}>
        <div className={styles.split}>
          <SourceList items={apiProviders} selected={selection.api} statuses={statuses} pending={query.isPending}
            onSelect={id => select("api", id)} onAdd={() => select("api", "")} />
          <section className={styles.detail}>
            {query.error && <p role="alert">{query.error.message}</p>}
            <p className={styles.note}>{t.providers.apiHint}</p>
            {form("api", API_KINDS, "openai_compat", true)}
          </section>
        </div>
      </div>}
    </div>
  </div>;
}
