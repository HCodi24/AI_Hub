import { useEffect, useState } from "react";
import { Menu, Folder, FileEdit } from "lucide-react";
import { useChatStore } from "./chat-store";
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
export function ChatToolbar({ config }: { config?: Config }) {
  const { settings, configure, initialize, streaming, setDrawer } = useChatStore(useShallow(s => ({ settings: s.settings, configure: s.configure, initialize: s.initialize, streaming: s.streaming, setDrawer: s.setDrawer })));
  const [memory, setMemory] = useState(false);
  const mutis = useQuery({ queryKey: ["mutis"], queryFn: () => mutisApi.get() });
  const client = useQueryClient();
  const provider = config?.providers.find((entry) => entry.id === (settings.provider_id === "mutis" ? mutis.data?.roles.lead.provider_id : settings.provider_id));
  const cli = provider?.kind.endsWith("_cli");
  useEffect(() => {
    if (config) initialize(config.default_folder, config.providers[0]?.id || "", config.providers[0]?.models[0] || "");
  }, [config, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === (id === "mutis" ? mutis.data?.roles.lead.provider_id : id));
    configure({ provider_id: id, model: next?.models[0] || "", file_tools: "none", permission: "write" });
  }
  return <header className={styles.toolbar}>
    <div className={styles.heading}><span className={styles.mobile}><IconButton aria-label={chatText.menu} onClick={() => setDrawer(true)}><Menu size={20} /></IconButton></span>
      <span>{chatText.local}</span>
      <button disabled={!settings.folder} onClick={() => setMemory(true)}><FileEdit size={16} aria-hidden="true" />{t.memory}</button></div>
    {memory && <MemoryPanel folder={settings.folder} close={() => setMemory(false)} />}
    <div className={styles.controls} role="group" aria-label={chatText.mode}>
      <Select label={chatText.source} value={settings.provider_id} disabled={streaming} onChange={(event) => changeProvider(event.target.value)}>
        <option value="">{chatText.source}</option><option value="mutis" disabled={!mutis.data?.enabled}>{t.mutis}</option>{config?.providers.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
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
    {settings.provider_id === "mutis" && mutis.data && <div className={styles.controls}>{(Object.keys(t.switches) as (keyof typeof t.switches)[]).map(key => <label key={key}><input type="checkbox" checked={mutis.data[key]} disabled={streaming} onChange={event => {
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
  </header>;
}
