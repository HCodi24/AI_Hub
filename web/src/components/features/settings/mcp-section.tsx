import { useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import type { McpServer } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { errorText, useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
const CODEGRAPH: McpServer = { name: "codegraph", command: "codegraph", args: ["serve", "--mcp"], env: {}, enabled: true };
const EMPTY = { name: "", command: "", args: "", env: "" };
const lines = (text: string) => text.split("\n").map(line => line.trim()).filter(Boolean);
const toEnv = (text: string) => Object.fromEntries(lines(text).flatMap(line => { const at = line.indexOf("="); return at > 0 ? [[line.slice(0, at).trim(), line.slice(at + 1)]] : []; }));
/** MCP servers MutiS runs itself for API sources; Codex and Claude keep their own. */
export function McpSection() {
  const { query, apply } = useSettingsConfig();
  const [form, setForm] = useState(EMPTY);
  const [tested, setTested] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState("");
  const servers = query.data?.mcp_servers ?? [];
  async function save(next: McpServer[]) {
    try { apply(await settingsApi.saveConfig({ mcp_servers: next })); notify(t.mcp.saved); return true; }
    catch (error) { notify(errorText(error, t.error)); return false; }
  }
  async function test(server: McpServer) {
    setBusy(server.name);
    try {
      const tools = await settingsApi.testMcp(server);
      const names = tools.map(tool => tool.read_only ? `${tool.name} (${t.mcp.readOnly})` : tool.name).join(", ") || "—";
      setTested(current => ({ ...current, [server.name]: t.mcp.tools(names) }));
    } catch (error) { setTested(current => ({ ...current, [server.name]: errorText(error, t.error) })); }
    finally { setBusy(""); }
  }
  async function add(server: McpServer) {
    if (await save([...servers.filter(item => item.name !== server.name), server])) setForm(EMPTY);
  }
  return <section className={styles.group} aria-label={t.mcp.title}><h3>{t.mcp.title}</h3>
    <p className={styles.note}>{t.mcp.intro}</p><p className={styles.note}>{t.mcp.readOnlyNote}</p><p className={styles.note}>{t.mcp.warn}</p>
    <ul className={styles.rows}>
      {servers.length === 0 && <li><span className={styles.rowText}>{t.mcp.none}</span></li>}
      {servers.map(server => <li key={server.name}>
        <span className={styles.rowText}><strong>{server.name}</strong><small title={[server.command, ...server.args].join(" ")}>{[server.command, ...server.args].join(" ")}</small>
          {tested[server.name] && <small role="status">{tested[server.name]}</small>}</span>
        <label className={styles.badge} data-on={server.enabled}><input type="checkbox" checked={server.enabled}
          onChange={event => void save(servers.map(item => item.name === server.name ? { ...item, enabled: event.target.checked } : item))} /> {t.mcp.enabled}</label>
        <Button size="sm" disabled={Boolean(busy)} onClick={() => void test(server)}>{busy === server.name ? t.mcp.testing : t.mcp.test}</Button>
        <Button size="sm" variant="ghost" aria-label={`${t.mcp.remove} ${server.name}`} onClick={() => void save(servers.filter(item => item.name !== server.name))}><Trash2 size={16} aria-hidden="true" /></Button>
      </li>)}
    </ul>
    <div className={styles.actions}>
      {!servers.some(server => server.name === "codegraph") && <Button size="sm" onClick={() => void add(CODEGRAPH)}><Plus size={16} aria-hidden="true" />{t.mcp.addCodegraph}</Button>}</div>
    <form className={styles.form} onSubmit={event => { event.preventDefault(); void add({ name: form.name.trim(), command: form.command.trim(), args: lines(form.args), env: toEnv(form.env), enabled: true }); }}>
      <label>{t.mcp.name}<input value={form.name} onChange={event => setForm({ ...form, name: event.target.value })} required /></label>
      <label>{t.mcp.command}<input value={form.command} onChange={event => setForm({ ...form, command: event.target.value })} required /></label>
      <label>{t.mcp.args}<textarea rows={2} value={form.args} onChange={event => setForm({ ...form, args: event.target.value })} /></label>
      <label>{t.mcp.env}<textarea rows={2} value={form.env} onChange={event => setForm({ ...form, env: event.target.value })} /></label>
      <Button type="submit" variant="primary"><Plus size={16} aria-hidden="true" />{t.mcp.add}</Button>
    </form>
  </section>;
}
