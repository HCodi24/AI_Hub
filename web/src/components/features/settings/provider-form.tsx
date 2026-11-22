import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ChevronDown, Loader2, PlugZap, Search, Trash2 } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Dropdown } from "@/components/select/select";
import { Switch } from "@/components/button/switch";
import { isModelVisible, useModelVisibility } from "@/lib/model-visibility";
import { antigravityModel } from "@/lib/antigravity-models";
import type { ConnectionTest, FullConfig, Provider, ProviderInput } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { AntigravityConnection, AntigravityProjectOptions } from "./antigravity-connection";
import { ClaudeConnection } from "./claude-connection";
import { CodexConnection } from "./codex-connection";
import { errorText } from "./use-settings";
import { useSettingsAutosave } from "./use-settings-autosave";
import styles from "./settings.module.css";
const KINDS = ["codex_cli", "claude_cli", "antigravity_cli", "openai_compat", "anthropic"] as const;
const isCli = (kind: string) => kind === "codex_cli" || kind === "claude_cli" || kind === "antigravity_cli";
const blank = (kind: string): ProviderInput => ({ name: "", kind, base_url: "", api_key: "", executable: "", models: [], agent_capable: false });
type Candidate = { path: string; version: string; source: string };
/** Group of a model id. Claude ids name their family (claude-sonnet-5 → sonnet, claude-3-5-haiku → haiku, opus[1m] → opus); others use the suffix after the last "-" (gpt-6.2-sol → sol); "" means none. */
function groupName(model: string, kind = ""): string {
  const id = model.replace(/\[.*\]$/, "");
  // Antigravity serves several vendors: group by vendor instead of family/suffix.
  if (kind === "antigravity_cli") return /^gemini/i.test(id) ? "gemini" : /^claude/i.test(id) ? "claude" : /^(gpt|chatgpt|openai|o\d)/i.test(id) ? "chatgpt" : "";
  if (/^claude-/i.test(id)) return id.split("-").slice(1).find(part => /^[a-z]+$/i.test(part)) ?? "";
  if (/^[a-z]+$/i.test(id)) return id;
  const cut = id.lastIndexOf("-");
  return cut > 0 && cut < id.length - 1 ? id.slice(cut + 1) : "";
}
/** Groups model ids in order of first appearance; ids without a group go last. */
function modelGroups(models: string[], nameOf = (model: string) => groupName(model)): { name: string; models: string[] }[] {
  const groups = new Map<string, string[]>();
  for (const model of models) {
    const name = nameOf(model);
    groups.set(name, [...(groups.get(name) ?? []), model]);
  }
  const named = [...groups].filter(([name]) => name).map(([name, list]) => ({ name, models: list }));
  const other = groups.get("");
  return other ? [...named, { name: t.providers.modelsOther, models: other }] : named;
}
type SafeProviderInput = Omit<ProviderInput, "api_key">;
const safeProvider = (input: ProviderInput): SafeProviderInput => ({ name: input.name, kind: input.kind, base_url: input.base_url,
  executable: input.executable, models: input.models, agent_capable: input.agent_capable });
const isSafeProvider = (value: unknown): value is SafeProviderInput => !!value && typeof value === "object" &&
  "name" in value && typeof value.name === "string" && "kind" in value && typeof value.kind === "string" &&
  "base_url" in value && typeof value.base_url === "string" && "executable" in value && typeof value.executable === "string" &&
  "models" in value && Array.isArray(value.models) && value.models.every(item => typeof item === "string") &&
  "agent_capable" in value && typeof value.agent_capable === "boolean" && !("api_key" in value);
export function ProviderForm({ provider, onSaved, onDeleted, onTested,
  allowedKinds = KINDS, defaultKind = "openai_compat", showConnections = true }: {
  provider: Provider | null;
  onSaved: (config: FullConfig, id: string) => void; onDeleted: (config: FullConfig) => void;
  onTested: (id: string, result: ConnectionTest) => void;
  /** Kind options offered for a new source; a saved source keeps its own kind. */
  allowedKinds?: readonly string[]; defaultKind?: string; showConnections?: boolean;
}) {
  const formRef = useRef<HTMLFormElement>(null);
  const client = useQueryClient();
  const hiddenModels = useModelVisibility(state => state.hiddenModels);
  const setModelVisible = useModelVisibility(state => state.setModelVisible);
  const apiKeyRef = useRef(""); const apiKeyRevision = useRef(0); const editsRevision = useRef(0);
  const [apiKey, setApiKey] = useState("");
  const [modelsText, setModelsText] = useState(() => (provider?.models || []).join("\n"));
  const [busy, setBusy] = useState(""); const [note, setNote] = useState(""); const [closedGroups, setClosedGroups] = useState<string[]>([]);
  const modelGroupsKey = provider ? `ai-hub:settings:model-groups:${provider.id}` : null;
  useEffect(() => {
    let groups: string[] = [];
    try {
      const saved: unknown = modelGroupsKey && JSON.parse(localStorage.getItem(modelGroupsKey) ?? "null");
      if (Array.isArray(saved)) groups = saved.filter((id): id is string => typeof id === "string");
    } catch {}
    setClosedGroups(groups);
  }, [modelGroupsKey]);
  const [candidates, setCandidates] = useState<Candidate[] | null>(null); const [confirmDelete, setConfirmDelete] = useState(false);
  const initial = safeProvider(provider ? { ...provider, api_key: "" } : blank(defaultKind));
  const autosave = useSettingsAutosave<SafeProviderInput>({ key: `ai-hub:settings:provider:${provider?.id ?? "new"}`, initial,
    validate: isSafeProvider,
    save: async value => {
      if (!provider) return value;
      const keyRevision = apiKeyRevision.current; const submittedKey = apiKeyRef.current;
      const config = await settingsApi.updateProvider(provider.id, { ...value, api_key: submittedKey });
      onSaved(config, provider.id); setNote("");
      if (submittedKey && keyRevision === apiKeyRevision.current) { apiKeyRef.current = ""; setApiKey(""); }
      return value;
    }, onError: error => setNote(errorText(error, t.error)) });
  const form = autosave.draft;
  const hydrate = autosave.hydrate;
  useEffect(() => {
    if (provider) hydrate(safeProvider(provider));
  }, [provider, hydrate]);
  useEffect(() => {
    setModelsText(current => {
      const parsed = current.split(/\r?\n/).map(model => model.trim()).filter(Boolean);
      return JSON.stringify(parsed) === JSON.stringify(form.models) ? current : form.models.join("\n");
    });
  }, [form.models]);
  const change = (update: (current: SafeProviderInput) => SafeProviderInput, immediate = false) => {
    editsRevision.current += 1;
    const valid = Boolean(provider) && Boolean(formRef.current?.checkValidity());
    autosave.update(update, immediate, valid);
  };
  const set = <K extends keyof SafeProviderInput>(key: K, value: SafeProviderInput[K], immediate = false) => change(current => ({ ...current, [key]: value }), immediate);
  const body = (value: SafeProviderInput = form, executable = value.executable): ProviderInput => ({ ...value, name: value.name.trim(), base_url: value.base_url.trim(),
    executable: executable.trim(), models: value.models.map(model => model.trim()).filter(Boolean), api_key: apiKeyRef.current });
  async function createOrFlush(executable?: string): Promise<string | null> {
    if (provider) return await autosave.flush() ? provider.id : null;
    const config = await settingsApi.createProvider(body(form, executable));
    const id = config.providers.at(-1)?.id || null;
    if (id) { onSaved(config, id); apiKeyRef.current = ""; setApiKey(""); }
    return id;
  }
  async function act(name: string, action: () => Promise<void>) {
    setBusy(name); setNote("");
    try { await action(); } catch (error) { setNote(errorText(error, t.error)); } finally { setBusy(""); }
  }
  async function test(id: string) { const result = await settingsApi.testProvider(id); onTested(id, result); setNote(result.message); }
  const pick = (candidate: Candidate) => act("discover", async () => {
    change(current => ({ ...current, executable: candidate.path })); setCandidates(null);
  });
  const cliName = form.kind === "codex_cli" ? "codex" : form.kind === "antigravity_cli" ? "agy" : "claude";
  const installHint = form.kind === "codex_cli" ? "npm i -g @openai/codex" : form.kind === "antigravity_cli" ? "irm https://antigravity.google/cli/install.ps1 | iex" : "npm i -g @anthropic-ai/claude-code";
  // Keep the saved kind selectable even when the current category does not offer it.
  const kinds = allowedKinds.includes(form.kind) ? allowedKinds : [form.kind, ...allowedKinds];
  // A web account source is mostly about its sign-in, so that section comes first there.
  const canLoadModels = form.kind === "codex_cli" || form.kind === "claude_cli" || form.kind === "antigravity_cli" || form.kind === "openai_compat";
  // A saved CLI source keeps its default name and kind, so those two fields are not shown.
  const fixedCli = Boolean(provider) && isCli(form.kind);
  const canDelete = provider && provider.kind !== "antigravity_cli" && !["codex-cli", "claude-cli", "antigravity-cli"].includes(provider.id);
  // Every saved source shows its models as read-only rows; an unsaved source keeps manual entry.
  const listedModels = Boolean(provider);
  const mode = provider?.web_login ? "web" : "machine";
  const connection = showConnections && provider && (provider.kind === "claude_cli"
    ? <ClaudeConnection key={provider.id} providerId={provider.id} mode={mode} />
    : provider.kind === "codex_cli" ? <CodexConnection key={provider.id} providerId={provider.id} mode={mode} />
    : provider.kind === "antigravity_cli" && provider.web_login ? <AntigravityConnection key={provider.id} providerId={provider.id} /> : null);
  function modelRow(model: string, ids: string[]) {
    return <li key={model} className={styles.modelRow}>
      <span>{model}</span>
      {provider && <Switch checked={ids.some(id => isModelVisible(hiddenModels, provider.id, id))}
        aria-label={t.providers.modelVisible(model)} onChange={event => {
          for (const id of ids) setModelVisible(provider.id, id, event.target.checked);
        }} />}
    </li>;
  }
  function modelRows(models: string[]) {
    if (form.kind !== "antigravity_cli") return models.map(model => modelRow(model, [model]));
    const rows = new Map<string, string[]>();
    for (const id of models) {
      const model = antigravityModel(id).id;
      const variants = rows.get(model) ?? [];
      variants.push(id);
      rows.set(model, variants);
    }
    return [...rows].map(([model, variants]) => modelRow(model, variants));
  }
  function modelToggle(name: string, listId: string) {
    const open = !closedGroups.includes(listId);
    const action = open ? t.providers.modelsCollapse : t.providers.modelsExpand;
    return <button type="button" className={styles.modelFold} aria-expanded={open} aria-controls={listId}
      aria-label={`${action}: ${name}`} title={action}
      onClick={() => {
        const next = open ? [...closedGroups, listId] : closedGroups.filter(id => id !== listId);
        setClosedGroups(next);
        try { if (modelGroupsKey) localStorage.setItem(modelGroupsKey, JSON.stringify(next)); } catch {}
      }}>
      <ChevronDown size={18} aria-hidden="true" className={styles.modelChevron} />
    </button>;
  }
  return <form ref={formRef} className={styles.form} onSubmit={event => { event.preventDefault(); void act("save", async () => {
    const id = await createOrFlush();
    if (id) setNote(t.providers.savedProvider);
  }); }}>
    {provider?.web_login && connection}
    {!fixedCli && <label>{t.providers.name}<input required value={form.name} onChange={event => set("name", event.target.value)} /></label>}
    {!fixedCli && <label>{t.providers.kind}<Dropdown value={form.kind} disabled={Boolean(provider)} onChange={event => set("kind", event.target.value)}>
      {kinds.map(kind => <option key={kind} value={kind}>{t.providers.kinds[kind as keyof typeof t.providers.kinds] || kind}</option>)}</Dropdown></label>}
    {!isCli(form.kind) && <>
      <label>{t.providers.baseUrl}<input value={form.base_url} spellCheck={false} onChange={event => set("base_url", event.target.value)} /></label>
      <label>{t.providers.apiKey}<input type="password" autoComplete="off" value={apiKey}
        placeholder={provider?.api_key ? t.providers.apiKeyKept : ""} onChange={event => {
          apiKeyRef.current = event.target.value; apiKeyRevision.current += 1;
          setApiKey(event.target.value); change(current => current);
        }} /></label>
    </>}
    {isCli(form.kind) && <label>{t.providers.executable}
      <span className={styles.inline}><input value={form.executable} spellCheck={false} placeholder={t.providers.executableHint}
        onChange={event => set("executable", event.target.value)} />
      <Button disabled={Boolean(busy)} onClick={() => void act("discover", async () => {
        const found = await settingsApi.discoverCli(cliName);
        if (found.length === 1 && found[0]) { await pick(found[0]); } else setCandidates(found);
      })}><Search size={16} aria-hidden="true" />{busy === "discover" ? t.providers.discovering : t.providers.discover}</Button></span></label>}
    {candidates && (candidates.length ? <div className={styles.candidates}><strong>{t.providers.pickInstall}</strong>
      {candidates.map(candidate => <button type="button" key={candidate.path} onClick={() => void pick(candidate)}>
        {candidate.path} · {candidate.version} · {candidate.source}</button>)}</div>
      : <p className={styles.note}>{t.providers.notFound(installHint)} <Button size="sm" onClick={() => void navigator.clipboard.writeText(installHint)}>{t.providers.copy}</Button></p>)}
    {listedModels ? <div className={styles.modelBlock}>
      <span>{t.providers.modelsListed}</span>
      <div id="provider-models">
      {form.models.length ? modelGroups(form.models, model => groupName(model, form.kind)).map(group => {
        const listId = `provider-models-${group.name}`;
        const hasFamilies = form.kind === "antigravity_cli" && (group.name === "claude" || group.name === "gemini");
        const familyOf = group.name === "gemini"
          ? (model: string) => model.match(/^gemini-\d+(?:\.\d+)*-([a-z][a-z0-9]*)(?:-|$)/i)?.[1]?.toLowerCase() ?? ""
          : undefined;
        return <section key={group.name} className={styles.modelGroup}>
          <div className={styles.modelGroupHead}><strong>{group.name}</strong>{modelToggle(group.name, listId)}</div>
          <ul id={listId} className={styles.modelList} hidden={closedGroups.includes(listId)}>
            {hasFamilies ? modelGroups(group.models, familyOf).map(family => {
              const familyId = `${listId}-${encodeURIComponent(family.name)}`;
              return <li key={family.name} className={styles.modelFamily}>
                <div className={styles.modelSub}><strong>{family.name}</strong>{modelToggle(family.name, familyId)}</div>
                <ul id={familyId} className={styles.modelFamilyList} hidden={closedGroups.includes(familyId)}>
                  {modelRows(family.models)}
                </ul>
              </li>;
            }) : modelRows(group.models)}
          </ul>
        </section>;
      })
        : <p className={styles.modelEmpty}>{canLoadModels ? t.providers.modelsEmpty : t.providers.modelsNone}</p>}</div></div>
      : <label>{t.providers.models}<textarea rows={4} spellCheck={false} value={modelsText} onChange={event => {
        setModelsText(event.target.value);
        set("models", event.target.value.split(/\r?\n/).map(model => model.trim()).filter(Boolean));
      }} /></label>}
    {form.kind === "openai_compat" && <label className={styles.check}><Switch checked={form.agent_capable}
      onChange={event => set("agent_capable", event.target.checked, true)} /><span>{t.providers.agent} <small>{t.providers.agentHint}</small></span></label>}
    <div className={styles.actions}>
      {!provider && <Button type="submit" variant="primary" disabled={Boolean(busy) || autosave.busy}>{busy === "save" && <Loader2 size={16} className="spin" aria-hidden="true" />}{t.save}</Button>}
      {canLoadModels && <Button type="button" variant="primary" disabled={Boolean(busy) || !provider} onClick={() => void act("models", async () => {
        if (!provider) return;
        await autosave.exclusive(async () => {
          const revision = editsRevision.current;
          const loaded = await settingsApi.models(provider.id);
          void client.invalidateQueries({ queryKey: ["config"] });
          if (editsRevision.current === revision) {
            setModelsText(loaded.models.join("\n"));
            change(current => ({ ...current, models: loaded.models }));
            setNote(t.providers.loadedModels(loaded.models.length));
          }
        });
      })}>{t.providers.loadModels}</Button>}
      <Button type="button" disabled={Boolean(busy) || !provider} onClick={() => void act("test", async () => {
        if (provider) await autosave.exclusive(() => test(provider.id));
      })}>
        <PlugZap size={16} aria-hidden="true" />{busy === "test" ? t.providers.testing : t.providers.test}</Button>
      {canDelete && <Button type="button" variant="danger" disabled={Boolean(busy)} onClick={() => setConfirmDelete(true)}><Trash2 size={16} aria-hidden="true" />{t.delete}</Button>}
    </div>
    {confirmDelete && canDelete && provider && <div className={styles.confirm} role="group"><p>{t.providers.confirmDelete(provider.name)}</p>
      <Button type="button" variant="danger" onClick={() => void act("delete", async () => {
        await autosave.exclusive(async () => onDeleted(await settingsApi.deleteProvider(provider.id)));
      })}>{t.delete}</Button>
      <Button type="button" onClick={() => setConfirmDelete(false)}>{t.cancel}</Button></div>}
    {note && <p className={styles.note} role="status">{note}</p>}
    {!provider?.web_login && connection}
    {showConnections && provider?.kind === "antigravity_cli" && <AntigravityProjectOptions />}
  </form>;
}
