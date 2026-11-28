import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, KeyRound, Plus, Terminal } from "lucide-react";
import { claudeApi, safeClaudeLoginUrl } from "@/apis/claude.api";
import { codexApi, safeCodexLoginUrl } from "@/apis/codex.api";
import { settingsApi } from "@/apis/settings.api";
import { SOURCE_LOGOS } from "@/constants/source-logos";
import { Button } from "@/components/button/button";
import { PillTabs } from "@/components/pill-tabs/pill-tabs";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import { ApiError } from "@/lib/http";
import { useProviderConnectionQueries } from "@/lib/provider-connections";
import type { ConnectionTest, FullConfig, Provider } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { planLabel } from "./account-parts";
import { LoginOutButton } from "./login_out";
import { ProviderForm } from "./provider-form";
import { useAutoLoadModels } from "./use-auto-load-models";
import { errorText, useSettingsConfig } from "../use-settings";
import styles from "../settings.module.css";

const SUB_TABS = ["cli", "login", "api"] as const;
type SubTab = (typeof SUB_TABS)[number];
const CLI_KINDS = ["codex_cli", "claude_cli"] as const;
const API_KINDS = ["openai_compat", "anthropic"] as const;
const MACHINE_KINDS = [...CLI_KINDS, "antigravity_cli"] as const;
type MachineKind = (typeof MACHINE_KINDS)[number];
const CLI_INSTALL_GUIDES: Record<MachineKind, string> = {
  codex_cli: "https://learn.chatgpt.com/docs/codex/cli",
  claude_cli: "https://code.claude.com/docs/en/setup",
  antigravity_cli: "https://antigravity.google/docs/cli-getting-started?hl=en",
};
const isCliKind = (kind: string) => kind === "codex_cli" || kind === "claude_cli" || kind === "antigravity_cli";
/** CLI tab: CLI sources using this machine's account; "Web": CLI sources signed in through the web. */
const groupOf = (provider: Provider): SubTab => !isCliKind(provider.kind) ? "api" : provider.web_login ? "login" : "cli";
/** Provider id per subtab; "" = new draft, null = nothing picked yet. */
type Selection = Record<SubTab, string | null>;
const NO_SELECTION: Selection = { cli: null, login: null, api: null };

function SourceLogo({ kind, size = 16 }: { kind: string; size?: number }) {
  const [missing, setMissing] = useState(false);
  if (!isCliKind(kind)) return <KeyRound size={size} />;
  const logo = SOURCE_LOGOS[kind];
  if (missing || !logo) return <Terminal size={size} />;
  return <img className={styles.sourceLogo} src={logo} alt="" onError={() => setMissing(true)} />;
}

/** Full-width card of sources, one row per source (same look as the skill card). */
function SourceCard({ items, selected, statuses, pending, tagOf, emailOf, summaryOf, onSelect, actionOf, children }: {
  items: readonly Provider[]; selected: string | null; statuses: Record<string, ConnectionTest | undefined>;
  pending: boolean; tagOf?: (provider: Provider) => { text: string; title?: string } | null; onSelect: (id: string) => void;
  /** Signed-in account email shown after "Đã đăng nhập". */
  emailOf?: (provider: Provider) => string | null;
  /** Custom status summary where the account labels do not apply. */
  summaryOf?: (provider: Provider, status?: ConnectionTest) => { text: string; state: "ok" | "error" | "unknown" } | undefined;
  /** Control shown at the right end of a row. */
  actionOf?: (provider: Provider) => ReactNode;
  /** Extra rows after the sources (e.g. not-yet-created web accounts). */
  children?: ReactNode;
}) {
  if (pending) return <Skeleton />;
  if (!items.length && !children) return null;
  return <ul className={styles.addonCard} aria-label={t.providers.list}>
    {items.map(provider => { const status = statuses[provider.id]; const tag = tagOf?.(provider);
      // Account rows only say signed in or not; waiting/failed sign-ins count as not signed in.
      const email = status?.ok ? emailOf?.(provider) : null;
      const summary = summaryOf?.(provider, status);
      const message = summary?.text ?? (status?.ok ? (email ? `${t.providers.account.signedIn} · ${email}` : t.providers.account.signedIn) : t.providers.account.notSignedIn);
      const dotState = summary?.state ?? (status?.ok ? "ok" : "unknown");
      return <li key={provider.id} className={styles.addonRow}>
        <button type="button" className={styles.sourceRow} aria-current={provider.id === selected ? "true" : undefined}
          onClick={() => onSelect(provider.id)}>
          <span className={styles.addonIcon} aria-hidden="true"><SourceLogo kind={provider.kind} /></span>
          <span className={styles.addonText}>
            <span className={styles.addonName}><strong>{provider.name}</strong>{tag && <span className={styles.planTag} title={tag.title}>{tag.text}</span>}</span>
            <small title={status?.message}><span className={styles.dot} data-state={dotState} aria-hidden="true" />
              <span>{message}</span></small>
          </span>
        </button>
        {actionOf && <span className={styles.addonControls}>{actionOf(provider)}</span>}
      </li>; })}
    {children}
  </ul>;
}

/** A source row that does not exist yet; "Đăng nhập" creates and signs it in, the row or "›" creates it and opens its page. */
function PlaceholderRow({ kind, name, disabled, onSignIn, onOpen }: {
  kind: string; name: string; disabled: boolean; onSignIn: () => void; onOpen: () => void;
}) {
  return <li className={styles.addonRow}>
    <button type="button" className={styles.sourceRow} disabled={disabled} onClick={onOpen}>
      <span className={styles.addonIcon} aria-hidden="true"><SourceLogo kind={kind} /></span>
      <span className={styles.addonText}>
        <span className={styles.addonName}><strong>{name}</strong></span>
        <small><span className={styles.dot} data-state="unknown" aria-hidden="true" /><span>{t.providers.account.notSignedIn}</span></small>
      </span>
    </button>
    <span className={styles.addonControls}>
      <LoginOutButton connected={false} checking={false} disabled={disabled} onClick={onSignIn} />
      <Button size="sm" variant="ghost" className={styles.iconButton} disabled={disabled} aria-label={t.providers.detail(name)}
        title={t.providers.detailShort} onClick={onOpen}><ChevronRight size={18} aria-hidden="true" /></Button>
    </span>
  </li>;
}

export function ProvidersTab() {
  const client = useQueryClient();
  const { query, apply } = useSettingsConfig();
  const [tab, setTab] = useState<SubTab>("cli");
  const [visited, setVisited] = useState<Record<SubTab, boolean>>({ cli: true, login: false, api: false });
  const [selection, setSelection] = useState<Selection>(NO_SELECTION);
  const [adding, setAdding] = useState(false);
  const [signingIn, setSigningIn] = useState<string | null>(null);
  const [loginError, setLoginError] = useState<{ tab: "cli" | "login"; kind: string; message: string; showInstall?: boolean } | null>(null);
  const providers = query.data?.providers || [];
  const tests = useProviderConnectionQueries(providers, provider => provider.kind === "antigravity_cli");
  const statuses: Record<string, ConnectionTest | undefined> = Object.fromEntries(
    providers.map((provider, index) => [provider.id, tests[index]?.data]),
  );
  const antigravityChecks = new Map(providers.flatMap((provider, index) =>
    provider.kind === "antigravity_cli" ? [[provider.id, tests[index]] as const] : [],
  ));
  const machineSummary = (provider: Provider, status?: ConnectionTest) => {
    const check = antigravityChecks.get(provider.id);
    if (check?.isFetching && !status?.ok) return { text: t.providers.testing, state: "unknown" as const };
    if (check?.isError) return { text: errorText(check.error, t.error), state: "error" as const };
    if (status?.status === "error" && check) return { text: status.message, state: "error" as const };
    return undefined;
  };
  // Plan (Plus, Pro, Max…) of each CLI source's account; same cache keys as the connection panels below.
  const cliSources = providers.filter(provider => isCliKind(provider.kind) && provider.kind !== "antigravity_cli");
  const accounts = useQueries({ queries: cliSources.map(provider => provider.kind === "codex_cli"
    // Polls while a browser sign-in started from a list row is waiting, so the row updates by itself.
    ? { queryKey: ["codex-connection", provider.id], queryFn: ({ signal }: { signal: AbortSignal }) => codexApi.status(provider.id, signal),
        refetchInterval: (query: { state: { data?: { pending?: boolean } } }) => query.state.data?.pending ? 2000 : false }
    : { queryKey: ["claude-connection", provider.id], queryFn: ({ signal }: { signal: AbortSignal }) => claudeApi.status(provider.id, signal) }) });
  const cliModelLoadEntries = cliSources.map(({ id }, index) => {
    const account = accounts[index]?.data;
    return { id, connected: account?.connected, email: account?.email };
  });
  useAutoLoadModels([
    ...cliModelLoadEntries,
    ...providers.filter(provider => provider.kind === "antigravity_cli").map(({ id }) => {
      const status = statuses[id];
      return { id, connected: status?.ok, email: status?.email };
    }),
  ]);
  const plans: Record<string, string | null | undefined> = Object.fromEntries(cliSources.map((provider, index) => {
    const data = accounts[index]?.data;
    return [provider.id, data?.connected ? data.plan : null];
  }));
  // Antigravity reports its plan with the connection test (no separate status call).
  for (const provider of providers) if (provider.kind === "antigravity_cli" && statuses[provider.id]?.ok) plans[provider.id] = statuses[provider.id]?.plan;
  const emails: Record<string, string | null | undefined> = Object.fromEntries(cliSources.map((provider, index) => {
    const data = accounts[index]?.data;
    return [provider.id, data?.connected ? data.email : null];
  }));
  // Antigravity reports its email with the connection test (saved when the web account signed in).
  const emailOf = (provider: Provider) => emails[provider.id] || (provider.kind === "antigravity_cli" ? statuses[provider.id]?.email : null) || null;
  // CLI rows follow their account status directly, so a sign-in finished from the list shows without opening the source.
  const rowStatuses: Record<string, ConnectionTest | undefined> = { ...statuses };
  cliSources.forEach((provider, index) => {
    const data = accounts[index]?.data;
    if (data) rowStatuses[provider.id] = { ok: data.connected, status: data.connected ? "connected" : "not_configured", message: data.message, detail: "" };
  });
  const planTag = (provider: Provider) => {
    const plan = plans[provider.id];
    if (!plan) return null;
    const label = planLabel(plan);
    return { text: provider.kind === "antigravity_cli" ? label.replace(/^Google AI /i, "") : label };
  };
  const lists = useMemo<Record<SubTab, readonly Provider[]>>(() => {
    const result: Record<SubTab, Provider[]> = { cli: [], login: [], api: [] };
    for (const provider of providers) result[groupOf(provider)].push(provider);
    return result;
  }, [providers]);

  // Keep every category's selection valid after load, save and delete.
  useEffect(() => {
    setSelection(current => {
      let next = current;
      for (const id of SUB_TABS) {
        const value = current[id];
        if (value === "") continue; // open draft stays open
        if (value && lists[id].some(provider => provider.id === value)) continue;
        // All source cards stay unselected until the user opens a row.
        const fallback = null;
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

  const onSaved = (id: SubTab) => (config: FullConfig, providerId: string) => { apply(config); select(id, providerId); };
  const onDeleted = (id: SubTab) => (config: FullConfig) => {
    apply(config);
    select(id, null);
  };
  const onTested = (providerId: string, result: ConnectionTest) => client.setQueryData(["provider-test", providerId], result);

  function form(id: SubTab, allowedKinds: readonly string[], defaultKind: string, showConnections: boolean) {
    const list = lists[id];
    const value = selection[id];
    const current = value ? list.find(provider => provider.id === value) ?? null : null;
    if (value === null) return <p className={styles.note}>{t.providers.selectHint}</p>;
    if (value !== "" && !current) return <p className={styles.note}>{t.providers.selectHint}</p>;
    return <ProviderForm key={`${id}:${value || "new"}`} provider={current}
      allowedKinds={allowedKinds} defaultKind={defaultKind} showConnections={showConnections}
      onSaved={onSaved(id)} onDeleted={onDeleted(id)} onTested={onTested} />;
  }

  /** Creates a CLI source (`web` = signs in through the browser, otherwise uses this machine's CLI account).
   *  It reuses the program and models of an existing source of the same CLI; an empty program means "find it on the machine". */
  async function addAccount(kind: string, web = true): Promise<string> {
    const template = providers.find(provider => provider.kind === kind);
    const base = web ? t.providers.account.webName(kind) : t.providers.account.cliName(kind);
    const count = lists[web ? "login" : "cli"].filter(provider => provider.kind === kind).length;
    // The first one keeps the name of the default row ("Codex"), later ones are numbered.
    const name = count ? t.providers.account.addName(base, count + 1) : base;
    const config = await settingsApi.createProvider({ name, kind, base_url: "", api_key: "",
      executable: template?.executable ?? "", models: template?.models ?? [], agent_capable: false, web_login: web });
    apply(config);
    const known = new Set(providers.map(provider => provider.id));
    const created = config.providers.find(provider => !known.has(provider.id));
    if (!created) throw new Error(t.error);
    return created.id;
  }

  /** Row "Đăng nhập": creates the web source if needed, then starts its browser sign-in.
   *  In a regular browser, reserve the popup during the click. */
  async function signInRow(kind: string, providerId?: string) {
    if (adding) return;
    const desktopCodex = kind === "codex_cli" && "__TAURI__" in window;
    const popup = desktopCodex ? null : window.open("about:blank", "_blank");
    if (popup) popup.opener = null;
    setAdding(true);
    setSigningIn(kind);
    setLoginError(null);
    let id = providerId;
    try {
      id ??= await addAccount(kind);
      let url: string | null;
      if (kind === "codex_cli") {
        const key = ["codex-connection", id];
        await client.cancelQueries({ queryKey: key });
        const result = await codexApi.action(id, "web");
        client.setQueryData(key, result);
        url = safeCodexLoginUrl(result.url);
        if (!url) throw new Error(result.message || t.providers.loginInvalidUrl);
      } else {
        const result = await settingsApi.loginStart(id);
        url = safeClaudeLoginUrl(result.url);
        if (!url) { await settingsApi.loginCancel(id); throw new Error(t.providers.loginInvalidUrl); }
        // Claude and Antigravity show the confirmation code form on the source page.
        client.setQueryData(["claude-login-session", id], { session_id: result.session_id, url });
      }
      if (desktopCodex) {
        // Tauri intercepts this navigation and opens it in the default browser.
        window.location.assign(url);
      } else {
        if (kind !== "codex_cli" || !popup || popup.closed) select("login", id);
        if (popup && !popup.closed) popup.location.href = url;
      }
    } catch (error) {
      popup?.close();
      setLoginError({ tab: "login", kind, message: errorText(error, t.error),
        showInstall: Boolean(id) && error instanceof ApiError && error.status === 404 });
    } finally {
      setSigningIn(null);
      setAdding(false);
    }
  }
  /** CLI sign-in imports the account from this machine. Sign-out on either tab removes only MutiS's copy. */
  async function accountRow(kind: string, providerId: string | undefined, action: "cli" | "logout", tab: "cli" | "login" = "cli") {
    if (adding) return;
    setAdding(true);
    setLoginError(null);
    let id = providerId;
    try {
      id ??= await addAccount(kind, false);
      if (kind === "antigravity_cli") {
        await client.cancelQueries({ queryKey: ["provider-test", id] });
        const result = await settingsApi.antigravityAction(id, action);
        if (result.ok || action === "logout") onTested(id, result);
        else setLoginError({ tab: "cli", kind, message: result.detail ? `${result.message}. ${result.detail}` : result.message });
        return;
      }
      const codex = kind === "codex_cli";
      const key = [codex ? "codex-connection" : "claude-connection", id];
      await Promise.all([
        client.cancelQueries({ queryKey: key }),
        tab === "login" ? client.cancelQueries({ queryKey: ["provider-test", id] }) : undefined,
      ]);
      const result = codex ? await codexApi.action(id, action) : await claudeApi.action(id, action);
      if (action === "cli" && !result.connected) {
        setLoginError({ tab: "cli", kind, message: result.message || t.error });
      }
      client.setQueryData(key, result);
      client.setQueryData(["provider-test", id], { ok: result.connected,
        status: result.connected ? "connected" : "not_configured", message: result.message, detail: "" });
      // The server may have saved the program it found; refresh the source list.
      await client.invalidateQueries({ queryKey: ["settings-config"] });
    } catch (error) {
      const message = errorText(error, t.error);
      if (action === "logout") notify(message);
      else setLoginError({ tab: "cli", kind, message,
        showInstall: Boolean(id) && error instanceof ApiError && error.status === 404 });
    }
    finally { setAdding(false); }
  }
  function loginAlert(id: "cli" | "login") {
    if (loginError?.tab !== id) return null;
    const { kind, message, showInstall } = loginError;
    return <div role="alert">
      <p>{t.providers.loginFailed(t.providers.account.webName(kind), message)}</p>
      {showInstall && isCliKind(kind) && <a href={CLI_INSTALL_GUIDES[kind]} target="_blank" rel="noreferrer">
        {t.providers.loginInstall(t.providers.account.cliName(kind))}
      </a>}
    </div>;
  }
  /** Placeholder row clicked: create its source, then open its page. */
  async function openPlaceholder(kind: string, id: SubTab) {
    if (adding) return;
    setAdding(true);
    try { select(id, await addAccount(kind, id === "login")); }
    catch (error) { notify(errorText(error, t.error)); }
    finally { setAdding(false); }
  }
  /** "›" next to the sign-in button: opens the source's own page. */
  const detailButton = (id: SubTab, provider: Provider) =>
    <Button size="sm" variant="ghost" className={styles.iconButton} aria-label={t.providers.detail(provider.name)}
      title={t.providers.detailShort} onClick={() => select(id, provider.id)}><ChevronRight size={18} aria-hidden="true" /></Button>;
  const machineAction = (provider: Provider) => {
    const connected = rowStatuses[provider.id]?.ok;
    const checking = !connected && antigravityChecks.get(provider.id)?.isFetching;
    return <>
      <LoginOutButton connected={Boolean(connected)} checking={Boolean(checking)} disabled={Boolean(adding || checking)}
        onClick={() => void accountRow(provider.kind, provider.id, connected ? "logout" : "cli")} />
      {detailButton("cli", provider)}</>;
  };
  const rowAction = (provider: Provider) => {
    const connected = Boolean(rowStatuses[provider.id]?.ok);
    return <>
      <LoginOutButton connected={connected} checking={false} disabled={adding}
        onClick={() => {
          if (connected) void accountRow(provider.kind, provider.id, "logout", "login");
          else void signInRow(provider.kind, provider.id);
        }} />
      {detailButton("login", provider)}</>;
  };

  // Source page: replaces the list until "‹ Nguồn AI" goes back.
  const pageTab = tab;
  const pageId = pageTab ? selection[pageTab] : null;
  if (pageTab && pageId !== null) {
    const current = pageId ? lists[pageTab].find(provider => provider.id === pageId) ?? null : null;
    const tag = current && isCliKind(current.kind) ? planTag(current) : null;
    const email = current && isCliKind(current.kind) && rowStatuses[current.id]?.ok ? emailOf(current) : null;
    const currentStatus = current ? rowStatuses[current.id] : undefined;
    const kindLabel = current ? t.providers.kinds[current.kind as keyof typeof t.providers.kinds] || current.kind : "";
    return <div className={`${styles.providers} ${styles.sourceDetailPage}`}>
      <nav className={styles.breadcrumb} aria-label={t.providers.list}>
        <button type="button" className={styles.breadcrumbLink} onClick={() => select(pageTab, null)}>{t.tabs.providers}</button>
        <ChevronRight size={18} aria-hidden="true" />
        <span className={styles.breadcrumbCurrent} aria-current="page">{current?.name ?? t.providers.add}</span>
      </nav>
      <header className={`${styles.addonHead} ${styles.providersHead}`}>
        <div className={styles.addonHeadText}>
          <h3 className={current ? `${styles.panelTitle} ${styles.titleWithLogo}` : styles.panelTitle}>
            {current && <span className={styles.titleLogo} aria-hidden="true"><SourceLogo kind={current.kind} size={20} /></span>}
            {current?.name ?? t.providers.add}
          </h3>
          {current && <p className={styles.pageMeta}>{tag && <span className={styles.planTag}>{tag.text}</span>}
            {pageTab === "api" && <span>{kindLabel}</span>}
            <span>{pageTab === "api" ? currentStatus?.message || t.providers.notTested
              : machineSummary(current, currentStatus)?.text ?? email ?? (currentStatus?.ok ? t.providers.account.signedIn : t.providers.account.notSignedIn)}</span></p>}
        </div>
      </header>
      <section className={styles.detail}>{form(pageTab, pageTab === "api" ? API_KINDS : MACHINE_KINDS,
        pageTab === "api" ? "openai_compat" : "codex_cli", true)}</section>
    </div>;
  }

  return <div className={styles.providers}>
    <header className={`${styles.addonHead} ${styles.providersHead}`}>
      <div className={styles.addonHeadText}><h3 className={styles.panelTitle}>{t.tabs.providers}</h3></div>
      <div className={styles.addonHeadActions}>
        {/* Hidden (not removed) on the Web tab so the header keeps its height and nothing below shifts. */}
        <Button size="sm" variant="primary" disabled={tab === "login"} aria-hidden={tab === "login" || undefined}
          style={tab === "login" ? { visibility: "hidden" } : undefined}
          onClick={() => { const id = tab === "api" ? "api" : "cli"; select(id, ""); }}>
          <Plus size={16} aria-hidden="true" />{t.providers.add}</Button>
      </div>
    </header>
    <PillTabs idPrefix="providers" label={t.providers.subTabsLabel} value={tab} onChange={open}
      tabs={SUB_TABS.map(id => ({ id, label: t.providers.subTabs[id] }))} />
    <div className={styles.subPanels}>
      {visited.cli && <div className={styles.subPanel} role="tabpanel" id="providers-panel-cli"
        aria-labelledby="providers-tab-cli" hidden={tab !== "cli"} tabIndex={0}>
        <div className={styles.stack}>
          {query.error && <p role="alert">{query.error.message}</p>}
          <p className={styles.note}>{t.providers.cliHint}</p>
          <SourceCard items={lists.cli} selected={null} statuses={rowStatuses} pending={query.isPending}
            onSelect={id => select("cli", id)} tagOf={planTag} emailOf={emailOf} summaryOf={machineSummary} actionOf={machineAction}>
            {/* New machine: one click finds the installed CLI and signs in with its account. */}
            {MACHINE_KINDS.filter(kind => !lists.cli.some(provider => provider.kind === kind)).map(kind =>
              <PlaceholderRow key={kind} kind={kind} name={t.providers.account.cliName(kind)} disabled={adding}
                onSignIn={() => void accountRow(kind, undefined, "cli")} onOpen={() => void openPlaceholder(kind, "cli")} />)}
          </SourceCard>
          {loginAlert("cli")}
        </div>
      </div>}
      {visited.login && <div className={styles.subPanel} role="tabpanel" id="providers-panel-login"
        aria-labelledby="providers-tab-login" hidden={tab !== "login"} tabIndex={0}>
        <div className={styles.stack}>
          {query.error && <p role="alert">{query.error.message}</p>}
          <p className={styles.note}>{t.providers.loginNote}</p>
          <SourceCard items={lists.login} selected={null} statuses={rowStatuses} pending={query.isPending}
            onSelect={id => select("login", id)} tagOf={planTag} emailOf={emailOf} actionOf={rowAction}>
            {MACHINE_KINDS.filter(kind => !lists.login.some(provider => provider.kind === kind)).map(kind =>
              <PlaceholderRow key={kind} kind={kind} name={t.providers.account.webName(kind)} disabled={adding} onSignIn={() => void signInRow(kind)} onOpen={() => void openPlaceholder(kind, "login")} />)}
          </SourceCard>
          {signingIn && <p className={styles.note} role="status">{t.providers.loginStarting(t.providers.account.webName(signingIn))}</p>}
          {loginAlert("login")}
        </div>
      </div>}
      {visited.api && <div className={styles.subPanel} role="tabpanel" id="providers-panel-api"
        aria-labelledby="providers-tab-api" hidden={tab !== "api"} tabIndex={0}>
        <div className={styles.stack}>
          {query.error && <p role="alert">{query.error.message}</p>}
          <p className={styles.note}>{t.providers.apiHint}</p>
          {!query.isPending && !lists.api.length && <p className={styles.note}>{t.providers.apiEmpty}</p>}
          <SourceCard items={lists.api} selected={null} statuses={statuses} pending={query.isPending}
            onSelect={id => select("api", id)}
            summaryOf={(_provider, status) => ({ text: status?.message || t.providers.notTested,
              state: !status ? "unknown" : status.ok ? "ok" : "error" })}
            actionOf={provider => detailButton("api", provider)} />
        </div>
      </div>}
    </div>
  </div>;
}
