import { useEffect, useRef, useState } from "react";
import { Menu, Folder, FileEdit, ChevronDown } from "lucide-react";
import { useChatStore } from "@/components/features/chat/chat-store";
import { Select } from "@/components/select/select";
import { IconButton } from "@/components/icon-button/icon-button";
import { chatText } from "@/text/chat";
import { EFFORTS, PERMISSIONS, FILE_TOOLS } from "@/constants/chat";
import { PermissionSchema, EffortSchema, FileToolsSchema } from "@/schema/chat.schema";
import type { Config } from "@/schema/config.schema";
import styles from "./chat-toolbar.module.css";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { mutisApi } from "@/apis/mutis.api";
import { SourceControls } from "@/components/features/plan/source-controls";
import { MemoryPanel } from "@/components/features/memory/memory-panel";
import { cardText as t } from "@/text/cards";
import { useShallow } from "zustand/react/shallow";
import { notify } from "@/components/toast/toast";

const MODEL_LABELS: Record<string, string> = {
  "gpt-6-luna": "GPT-6 Luna",
  "gpt-6.1-sol": "GPT-6.1 Sol",
  "gpt-6.1-terra": "GPT-6.1 Terra",
};
const CODEX_CLI_MODEL_SUGGESTIONS = ["gpt-6-luna", "gpt-6.1-sol", "gpt-6.1-terra"];

/** Thin top row: mobile menu, title, memory. */
export function ChatHeader() {
  const { folder, setDrawer } = useChatStore(useShallow(s => ({ folder: s.settings.folder, setDrawer: s.setDrawer })));
  const [memory, setMemory] = useState(false);
  return <header className={styles.header}>
    <span className={styles.mobile}><IconButton aria-label={chatText.menu} onClick={() => setDrawer(true)}><Menu size={20} /></IconButton></span>
    <span>{chatText.local}</span>
    <button disabled={!folder} onClick={() => setMemory(true)}><FileEdit size={16} aria-hidden="true" />{t.memory}</button>
    {memory && <MemoryPanel folder={folder} close={() => setMemory(false)} />}
  </header>;
}

/** Compact picker for the composer box: one summary line that opens the full settings upward. */
export function ChatToolbar({ config }: { config?: Config }) {
  const { settings, configure, initialize, streaming } = useChatStore(useShallow(s => ({ settings: s.settings, configure: s.configure, initialize: s.initialize, streaming: s.streaming })));
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const picker = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      if (picker.current?.open && !picker.current.contains(event.target as Node)) {
        picker.current.open = false;
        setModelMenuOpen(false);
      }
    }
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape" || !picker.current?.open) return;
      event.preventDefault();
      picker.current.open = false;
      setModelMenuOpen(false);
      picker.current.querySelector("summary")?.focus();
    }
    document.addEventListener("pointerdown", closeOutside, true);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside, true);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, []);
  const mutis = useQuery({ queryKey: ["mutis"], queryFn: () => mutisApi.get() });
  const client = useQueryClient();
  const provider = config?.providers.find((entry) => entry.id === (settings.provider_id === "mutis" ? mutis.data?.roles.lead.provider_id : settings.provider_id));
  const models = provider?.kind === "codex_cli"
    ? [...new Set([...provider.models, ...CODEX_CLI_MODEL_SUGGESTIONS])]
    : provider?.models || [];
  const cli = provider?.kind.endsWith("_cli");
  useEffect(() => {
    if (config) initialize(config.default_folder, config.providers[0]?.id || "", config.providers[0]?.models[0] || "");
  }, [config, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === (id === "mutis" ? mutis.data?.roles.lead.provider_id : id));
    configure({ provider_id: id, model: next?.models[0] || "", file_tools: "none", permission: "write" });
  }
  const name = settings.provider_id === "mutis" ? t.mutis : provider?.name || chatText.source;
  return <details ref={picker} className={styles.picker} onToggle={(event) => {
    if (event.target === event.currentTarget && !event.currentTarget.open) setModelMenuOpen(false);
  }}>
    <summary><span>{settings.model || name}</span> <span className={styles.effort}>{chatText[settings.effort]}</span><ChevronDown size={14} aria-hidden="true" /></summary>
    <div className={styles.panel}>
      <div className={styles.controls} role="group" aria-label={chatText.mode}>
        <Select label={chatText.source} value={settings.provider_id} disabled={streaming} onChange={(event) => changeProvider(event.target.value)}>
          <option value="">{chatText.source}</option><option value="mutis" disabled={!mutis.data?.enabled}>{t.mutis}</option>{config?.providers.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
        </Select>
        <div className={styles.modelPicker} onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setModelMenuOpen(false);
        }}>
          <div className={styles.field}><span>{chatText.model}</span><div className={styles.modelInput}>
            <input aria-label={chatText.model} value={settings.model} disabled={streaming} aria-haspopup="listbox" aria-expanded={modelMenuOpen}
              onFocus={() => setModelMenuOpen(true)} onKeyDown={(event) => { if (event.key === "Escape") setModelMenuOpen(false); }}
              onChange={(event) => configure({ model: event.target.value })} />
            <button type="button" aria-label={chatText.model} aria-expanded={modelMenuOpen} disabled={streaming}
              onClick={() => setModelMenuOpen((open) => !open)}><ChevronDown size={16} aria-hidden="true" /></button>
          </div></div>
          {modelMenuOpen && <div className={styles.modelOptions} role="listbox" aria-label={chatText.model}>
            {models.map((model) => {
              const label = MODEL_LABELS[model] || model;
              return <button key={model} type="button" role="option" aria-selected={settings.model === model}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => { configure({ model }); setModelMenuOpen(false); }}>
                <span>{label}</span><small>{model}</small>
              </button>;
            })}
          </div>}
        </div>
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
      {settings.provider_id === "mutis" && mutis.data && <div className={styles.switches}>{(Object.keys(t.switches) as (keyof typeof t.switches)[]).map(key => <label key={key}><input type="checkbox" checked={mutis.data[key]} disabled={streaming} onChange={event => {
        void mutisApi.put({ ...mutis.data!, [key]: event.target.checked }).then(() => client.invalidateQueries({ queryKey: ["mutis"] })).catch(error => notify(String(error)));
      }} />{t.switches[key]}</label>)}</div>}
      {settings.provider_id === "mutis" && mutis.data && <details><summary>{t.team}</summary>{(["lead", "big", "small"] as const).map(key => <section key={key}><strong>{key}</strong><SourceControls showPermission={false} providers={config?.providers || []}
        value={{ ...mutis.data!.roles[key], assignee: mutis.data!.roles[key].provider_id }} disabled={streaming} change={value => {
          const next = { ...mutis.data!, roles: { ...mutis.data!.roles, [key]: { provider_id: value.assignee, model: value.model || "", effort: value.effort } } };
          void mutisApi.put(next).then(() => client.invalidateQueries({ queryKey: ["mutis"] })).catch(error => notify(String(error)));
        }} /></section>)}</details>}
      <label className={styles.folder}><Folder size={16} aria-hidden="true" /><span className="sr-only">{chatText.folder}</span><input title={settings.folder} value={settings.folder} disabled={streaming} spellCheck={false}
        onChange={(event) => configure({ folder: event.target.value })} /></label>
      {provider && !cli && <span className={styles.note}>{chatText.noCommands}</span>}
    </div>
  </details>;
}
