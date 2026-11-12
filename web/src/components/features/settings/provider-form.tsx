import { useState } from "react";
import { Loader2, PlugZap, Search, Trash2 } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import type { ConnectionTest, FullConfig, Provider, ProviderInput } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { ClaudeLogin } from "./claude-login";
import { CodexConnection } from "./codex-connection";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
const KINDS = ["codex_cli", "claude_cli", "openai_compat", "anthropic"] as const;
const isCli = (kind: string) => kind === "codex_cli" || kind === "claude_cli";
const blank: ProviderInput = { name: "", kind: "openai_compat", base_url: "", api_key: "", executable: "", models: [], agent_capable: false };
type Candidate = { path: string; version: string; source: string };
export function ProviderForm({ provider, status, onSaved, onDeleted, onTested }: {
  provider: Provider | null; status?: ConnectionTest;
  onSaved: (config: FullConfig, id: string) => void; onDeleted: (config: FullConfig) => void;
  onTested: (id: string, result: ConnectionTest) => void;
}) {
  const [form, setForm] = useState<ProviderInput>(() => provider ? { ...provider, api_key: "" } : blank);
  const [models, setModels] = useState(() => (provider?.models || []).join("\n"));
  const [busy, setBusy] = useState(""); const [note, setNote] = useState("");
  const [candidates, setCandidates] = useState<Candidate[] | null>(null); const [confirmDelete, setConfirmDelete] = useState(false);
  const set = <K extends keyof ProviderInput>(key: K, value: ProviderInput[K]) => setForm(current => ({ ...current, [key]: value }));
  const body = (executable = form.executable): ProviderInput => ({ ...form, name: form.name.trim(), base_url: form.base_url.trim(),
    executable: executable.trim(), models: models.split(/\r?\n/).map(m => m.trim()).filter(Boolean) });
  async function save(executable?: string): Promise<string | null> {
    const input = body(executable);
    const config = provider ? await settingsApi.updateProvider(provider.id, input) : await settingsApi.createProvider(input);
    const id = provider?.id || config.providers.at(-1)?.id || null;
    if (id) onSaved(config, id);
    setForm(current => ({ ...current, api_key: "" }));
    return id;
  }
  async function act(name: string, action: () => Promise<void>) {
    setBusy(name); setNote("");
    try { await action(); } catch (error) { setNote(errorText(error, t.error)); } finally { setBusy(""); }
  }
  async function test(id: string) { const result = await settingsApi.testProvider(id); onTested(id, result); setNote(result.message); }
  const pick = (candidate: Candidate) => act("discover", async () => {
    set("executable", candidate.path); setCandidates(null);
    const id = await save(candidate.path); if (id) await test(id);
  });
  const cliName = form.kind === "codex_cli" ? "codex" : "claude";
  const installHint = form.kind === "codex_cli" ? "npm i -g @openai/codex" : "npm i -g @anthropic-ai/claude-code";
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void act("save", async () => { await save(); setNote(t.providers.savedProvider); }); }}>
    <label>{t.providers.name}<input required value={form.name} onChange={event => set("name", event.target.value)} /></label>
    <label>{t.providers.kind}<select value={form.kind} disabled={Boolean(provider)} onChange={event => set("kind", event.target.value)}>
      {KINDS.map(kind => <option key={kind} value={kind}>{t.providers.kinds[kind]}</option>)}</select></label>
    {!isCli(form.kind) && <>
      <label>{t.providers.baseUrl}<input value={form.base_url} spellCheck={false} onChange={event => set("base_url", event.target.value)} /></label>
      <label>{t.providers.apiKey}<input type="password" autoComplete="off" value={form.api_key}
        placeholder={provider?.api_key ? t.providers.apiKeyKept : ""} onChange={event => set("api_key", event.target.value)} /></label>
    </>}
    {isCli(form.kind) && <label>{t.providers.executable}
      <span className={styles.inline}><input value={form.executable} spellCheck={false} placeholder={t.providers.executableHint}
        onChange={event => set("executable", event.target.value)} />
      {(form.kind !== "codex_cli" || !provider) && <Button disabled={Boolean(busy)} onClick={() => void act("discover", async () => {
        const found = await settingsApi.discoverCli(cliName);
        if (found.length === 1 && found[0]) { await pick(found[0]); } else setCandidates(found);
      })}><Search size={16} aria-hidden="true" />{busy === "discover" ? t.providers.discovering : t.providers.discover}</Button>}</span></label>}
    {candidates && (candidates.length ? <div className={styles.candidates}><strong>{t.providers.pickInstall}</strong>
      {candidates.map(candidate => <button type="button" key={candidate.path} onClick={() => void pick(candidate)}>
        {candidate.path} · {candidate.version} · {candidate.source}</button>)}</div>
      : <p className={styles.note}>{t.providers.notFound(installHint)} <Button size="sm" onClick={() => void navigator.clipboard.writeText(installHint)}>{t.providers.copy}</Button></p>)}
    <label>{t.providers.models}<textarea rows={4} spellCheck={false} value={models} onChange={event => setModels(event.target.value)} /></label>
    {form.kind === "openai_compat" && <label className={styles.check}><input type="checkbox" checked={form.agent_capable}
      onChange={event => set("agent_capable", event.target.checked)} /><span>{t.providers.agent} <small>{t.providers.agentHint}</small></span></label>}
    <div className={styles.actions}>
      <Button type="submit" variant="primary" disabled={Boolean(busy)}>{busy === "save" && <Loader2 size={16} className="spin" aria-hidden="true" />}{t.save}</Button>
      <Button disabled={Boolean(busy)} onClick={() => void act("test", async () => { const id = await save(); if (id) await test(id); })}>
        <PlugZap size={16} aria-hidden="true" />{busy === "test" ? t.providers.testing : t.providers.test}</Button>
      {!isCli(form.kind) && <Button disabled={Boolean(busy)} onClick={() => void act("models", async () => {
        const id = await save(); if (!id) return;
        const result = await settingsApi.models(id); setModels(result.models.join("\n")); setNote(t.providers.loadedModels(result.models.length));
      })}>{t.providers.loadModels}</Button>}
      {provider && <Button variant="danger" disabled={Boolean(busy)} onClick={() => setConfirmDelete(true)}><Trash2 size={16} aria-hidden="true" />{t.delete}</Button>}
    </div>
    {confirmDelete && provider && <div className={styles.confirm} role="group"><p>{t.providers.confirmDelete(provider.name)}</p>
      <Button variant="danger" onClick={() => void act("delete", async () => onDeleted(await settingsApi.deleteProvider(provider.id)))}>{t.delete}</Button>
      <Button onClick={() => setConfirmDelete(false)}>{t.cancel}</Button></div>}
    {(note || status) && <p className={styles.note} role="status" title={status?.detail}>{note || status?.message}</p>}
    {provider?.kind === "claude_cli" && status && !status.ok && <ClaudeLogin providerId={provider.id} onDone={() => void act("test", () => test(provider.id))} />}
    {provider?.kind === "codex_cli" && <CodexConnection key={provider.id} providerId={provider.id} />}
  </form>;
}
