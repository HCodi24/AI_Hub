import { useEffect, useRef, useState } from "react";
import { Loader2, PlugZap, Search, Trash2 } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Switch } from "@/components/button/switch";
import type { ConnectionTest, FullConfig, Provider, ProviderInput } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { ClaudeConnection } from "./claude-connection";
import { CodexConnection } from "./codex-connection";
import { errorText } from "./use-settings";
import { useSettingsAutosave } from "./use-settings-autosave";
import styles from "./settings.module.css";
const KINDS = ["codex_cli", "claude_cli", "openai_compat", "anthropic"] as const;
const isCli = (kind: string) => kind === "codex_cli" || kind === "claude_cli";
const blank = (kind: string): ProviderInput => ({ name: "", kind, base_url: "", api_key: "", executable: "", models: [], agent_capable: false });
type Candidate = { path: string; version: string; source: string };
type SafeProviderInput = Omit<ProviderInput, "api_key">;
const safeProvider = (input: ProviderInput): SafeProviderInput => ({ name: input.name, kind: input.kind, base_url: input.base_url,
  executable: input.executable, models: input.models, agent_capable: input.agent_capable });
const isSafeProvider = (value: unknown): value is SafeProviderInput => !!value && typeof value === "object" &&
  "name" in value && typeof value.name === "string" && "kind" in value && typeof value.kind === "string" &&
  "base_url" in value && typeof value.base_url === "string" && "executable" in value && typeof value.executable === "string" &&
  "models" in value && Array.isArray(value.models) && value.models.every(item => typeof item === "string") &&
  "agent_capable" in value && typeof value.agent_capable === "boolean" && !("api_key" in value);
export function ProviderForm({ provider, status, onSaved, onDeleted, onTested,
  allowedKinds = KINDS, defaultKind = "openai_compat", showConnections = true }: {
  provider: Provider | null; status?: ConnectionTest;
  onSaved: (config: FullConfig, id: string) => void; onDeleted: (config: FullConfig) => void;
  onTested: (id: string, result: ConnectionTest) => void;
  /** Kind options offered for a new source; a saved source keeps its own kind. */
  allowedKinds?: readonly string[]; defaultKind?: string; showConnections?: boolean;
}) {
  const formRef = useRef<HTMLFormElement>(null);
  const apiKeyRef = useRef(""); const apiKeyRevision = useRef(0); const editsRevision = useRef(0);
  const [apiKey, setApiKey] = useState("");
  const [modelsText, setModelsText] = useState(() => (provider?.models || []).join("\n"));
  const [busy, setBusy] = useState(""); const [note, setNote] = useState("");
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
  const cliName = form.kind === "codex_cli" ? "codex" : "claude";
  const installHint = form.kind === "codex_cli" ? "npm i -g @openai/codex" : "npm i -g @anthropic-ai/claude-code";
  // Keep the saved kind selectable even when the current category does not offer it.
  const kinds = allowedKinds.includes(form.kind) ? allowedKinds : [form.kind, ...allowedKinds];
  // A web account source is mostly about its sign-in, so that section comes first there.
  const mode = provider?.web_login ? "web" : "machine";
  const connection = showConnections && provider && (provider.kind === "claude_cli"
    ? <ClaudeConnection key={provider.id} providerId={provider.id} mode={mode} />
    : provider.kind === "codex_cli" ? <CodexConnection key={provider.id} providerId={provider.id} mode={mode} /> : null);
  return <form ref={formRef} className={styles.form} onSubmit={event => { event.preventDefault(); void act("save", async () => {
    const id = await createOrFlush();
    if (id) setNote(t.providers.savedProvider);
  }); }}>
    {provider?.web_login && connection}
    <label>{t.providers.name}<input required value={form.name} onChange={event => set("name", event.target.value)} /></label>
    <label>{t.providers.kind}<select value={form.kind} disabled={Boolean(provider)} onChange={event => set("kind", event.target.value)}>
      {kinds.map(kind => <option key={kind} value={kind}>{t.providers.kinds[kind as keyof typeof t.providers.kinds] || kind}</option>)}</select></label>
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
    <label>{t.providers.models}<textarea rows={4} spellCheck={false} value={modelsText} onChange={event => {
      setModelsText(event.target.value);
      set("models", event.target.value.split(/\r?\n/).map(model => model.trim()).filter(Boolean));
    }} /></label>
    {form.kind === "openai_compat" && <label className={styles.check}><Switch checked={form.agent_capable}
      onChange={event => set("agent_capable", event.target.checked, true)} /><span>{t.providers.agent} <small>{t.providers.agentHint}</small></span></label>}
    <div className={styles.actions}>
      <Button type="submit" variant="primary" disabled={Boolean(busy) || autosave.busy}>{busy === "save" && <Loader2 size={16} className="spin" aria-hidden="true" />}{t.save}</Button>
      <Button type="button" disabled={Boolean(busy) || !provider} onClick={() => void act("test", async () => {
        if (provider) await autosave.exclusive(() => test(provider.id));
      })}>
        <PlugZap size={16} aria-hidden="true" />{busy === "test" ? t.providers.testing : t.providers.test}</Button>
      {!isCli(form.kind) && <Button type="button" disabled={Boolean(busy) || !provider} onClick={() => void act("models", async () => {
        if (!provider) return;
        const result = await autosave.exclusive(async () => {
          const revision = editsRevision.current;
          const loaded = await settingsApi.models(provider.id);
          if (editsRevision.current === revision) {
            setModelsText(loaded.models.join("\n"));
            change(current => ({ ...current, models: loaded.models }));
            setNote(t.providers.loadedModels(loaded.models.length));
          }
        });
      })}>{t.providers.loadModels}</Button>}
      {provider && <Button type="button" variant="danger" disabled={Boolean(busy)} onClick={() => setConfirmDelete(true)}><Trash2 size={16} aria-hidden="true" />{t.delete}</Button>}
    </div>
    {confirmDelete && provider && <div className={styles.confirm} role="group"><p>{t.providers.confirmDelete(provider.name)}</p>
      <Button type="button" variant="danger" onClick={() => void act("delete", async () => {
        await autosave.exclusive(async () => onDeleted(await settingsApi.deleteProvider(provider.id)));
      })}>{t.delete}</Button>
      <Button type="button" onClick={() => setConfirmDelete(false)}>{t.cancel}</Button></div>}
    {(note || status) && <p className={styles.note} role="status" title={status?.detail}>{note || status?.message}</p>}
    {!provider?.web_login && connection}
  </form>;
}
