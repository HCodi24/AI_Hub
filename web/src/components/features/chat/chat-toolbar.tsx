import { useEffect } from "react";
import { Menu, SunMoon } from "lucide-react";
import { useChatStore } from "./chat-store";
import { Select } from "@/components/select/select";
import { IconButton } from "@/components/icon-button/icon-button";
import { chatText } from "@/text/chat";
import { EFFORTS, PERMISSIONS, FILE_TOOLS } from "@/constants/chat";
import { PermissionSchema, EffortSchema, FileToolsSchema } from "@/schema/chat.schema";
import type { Config } from "@/schema/config.schema";
import styles from "./chat-toolbar.module.css";
export function ChatToolbar({ config }: { config?: Config }) {
  const { settings, configure, initialize, streaming, setDrawer } = useChatStore();
  const provider = config?.providers.find((entry) => entry.id === settings.provider_id);
  const cli = provider?.kind.endsWith("_cli");
  useEffect(() => {
    if (config) initialize(config.default_folder, config.providers[0]?.id || "", config.providers[0]?.models[0] || "");
  }, [config, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === id);
    configure({ provider_id: id, model: next?.models[0] || "", file_tools: "none", permission: "write" });
  }
  function toggleTheme() {
    const current = document.documentElement.dataset.theme || (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    const theme = current === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("ai-hub-theme", theme); } catch { /* Storage can be unavailable. */ }
  }
  useEffect(() => {
    try { const theme = localStorage.getItem("ai-hub-theme"); if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme; } catch { /* Use system theme. */ }
  }, []);
  return <header className={styles.toolbar}>
    <div className={styles.heading}><span className={styles.mobile}><IconButton aria-label={chatText.menu} onClick={() => setDrawer(true)}><Menu size={20} /></IconButton></span>
      <span>{chatText.local}</span><IconButton aria-label={chatText.theme} onClick={toggleTheme}><SunMoon size={18} /></IconButton></div>
    <div className={styles.controls}>
      <Select label={chatText.source} value={settings.provider_id} disabled={streaming} onChange={(event) => changeProvider(event.target.value)}>
        <option value="">{chatText.source}</option>{config?.providers.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </Select>
      <label className={styles.field}><span>{chatText.model}</span><input list="chat-models" value={settings.model} disabled={streaming}
        onChange={(event) => configure({ model: event.target.value })} /><datalist id="chat-models">{provider?.models.map((model) => <option key={model} value={model} />)}</datalist></label>
      <Select label={chatText.permission} value={settings.permission} disabled={streaming} onChange={(event) => configure({ permission: PermissionSchema.parse(event.target.value) })}>
        {PERMISSIONS.map((permission) => <option key={permission} value={permission} disabled={permission === "full" && !cli}>{chatText[permission]}</option>)}
      </Select>
      <Select label={chatText.effort} value={settings.effort} disabled={streaming} onChange={(event) => configure({ effort: EffortSchema.parse(event.target.value) })}>
        {EFFORTS.map((effort) => <option key={effort} value={effort}>{chatText[effort]}</option>)}
      </Select>
      {provider?.agent_capable && <Select label={chatText.fileTools} value={settings.file_tools} disabled={streaming}
        onChange={(event) => configure({ file_tools: FileToolsSchema.parse(event.target.value) })}>
        {FILE_TOOLS.map((tools) => <option key={tools} value={tools}>{chatText[tools]}</option>)}
      </Select>}
    </div>
    <label className={styles.folder}><span>{chatText.folder}</span><input value={settings.folder} disabled={streaming} spellCheck={false}
      onChange={(event) => configure({ folder: event.target.value })} /></label>
    {provider && !cli && <span className={styles.note}>{chatText.noCommands}</span>}
  </header>;
}
