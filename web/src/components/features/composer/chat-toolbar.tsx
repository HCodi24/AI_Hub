import { useEffect, useRef, useState } from "react";
import { Menu, ChevronDown, RotateCcw } from "lucide-react";
import { useChatStore } from "@/components/features/chat/chat-store";
import { IconButton } from "@/components/icon-button/icon-button";
import { chatText } from "@/text/chat";
import { EFFORTS } from "@/constants/chat";
import type { Config } from "@/schema/config.schema";
import styles from "./chat-toolbar.module.css";
import { CliLimits } from "./cli-limits";
import { useQuery } from "@tanstack/react-query";
import { SourceMenu } from "./source-menu";
import { ModelMenu, modelLabel as getModelLabel } from "./model-menu";
import { mutisApi } from "@/apis/mutis.api";
import { PermissionMenu } from "./permission-menu";
import { cardText as t } from "@/text/cards";
import { useShallow } from "zustand/react/shallow";

const SLIDER_EFFORTS: readonly (typeof EFFORTS)[number][] = EFFORTS.filter((effort) => effort !== "default");
const CLI_MODEL_SUGGESTIONS: Record<string, string[]> = {
  codex_cli: ["gpt-6-luna", "gpt-6.1-sol", "gpt-6.1-terra"],
  claude_cli: ["opus", "sonnet", "haiku", "claude-fable-5-2"],
};

/** Sidebar access on mobile. */
export function ChatHeader() {
  const setDrawer = useChatStore(s => s.setDrawer);
  return <header className={styles.header}>
    <IconButton aria-label={chatText.menu} onClick={() => setDrawer(true)}><Menu size={20} /></IconButton>
  </header>;
}

/** Compact picker for the composer box: one summary line that opens the full settings upward. */
export function ChatToolbar({ config }: { config?: Config }) {
  const { settings, configure, initialize, streaming } = useChatStore(useShallow(s => ({ settings: s.settings, configure: s.configure, initialize: s.initialize, streaming: s.streaming })));
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [sourceMenuOpen, setSourceMenuOpen] = useState(false);
  const picker = useRef<HTMLDetailsElement>(null);
  const permissions = useRef<HTMLDetailsElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      if (permissions.current?.open && !permissions.current.contains(event.target as Node)) permissions.current.open = false;
      if (picker.current?.open && !picker.current.contains(event.target as Node)) {
        picker.current.open = false;
        setModelMenuOpen(false);
      }
    }
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      if (permissions.current?.open) {
        event.preventDefault();
        permissions.current.open = false;
        permissions.current.querySelector("summary")?.focus();
        return;
      }
      if (!picker.current?.open) return;
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
  const provider = config?.providers.find((entry) => entry.id === (settings.provider_id === "mutis" ? mutis.data?.roles.lead.provider_id : settings.provider_id));
  const models = provider ? [...new Set([...provider.models, ...(CLI_MODEL_SUGGESTIONS[provider.kind] || [])])] : [];
  const cli = provider?.kind.endsWith("_cli");
  useEffect(() => {
    if (config) initialize(config.default_folder, config.providers[0]?.id || "", config.providers[0]?.models[0] || "");
  }, [config, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === (id === "mutis" ? mutis.data?.roles.lead.provider_id : id));
    configure({ provider_id: id, model: next?.models[0] || "", file_tools: "none", permission: "write" });
  }
  const name = settings.provider_id === "mutis" ? t.mutis : provider?.name || chatText.source;
  const modelLabel = getModelLabel(settings.model) || name;
  const effortIndex = Math.max(0, SLIDER_EFFORTS.indexOf(settings.effort));
  return <><PermissionMenu value={settings.permission} disabled={streaming} cli={!!cli} menuRef={permissions}
    onOpen={() => { if (picker.current) picker.current.open = false; }} onPick={permission => configure({ permission })} />
  <details ref={picker} className={styles.picker} onToggle={(event) => {
    if (event.target === event.currentTarget && event.currentTarget.open && permissions.current) permissions.current.open = false;
    if (event.target === event.currentTarget && !event.currentTarget.open) { setModelMenuOpen(false); setSourceMenuOpen(false); }
  }}>
    <summary><span>{modelLabel}</span> <span className={styles.effort}>{chatText[settings.effort]}</span><ChevronDown size={14} aria-hidden="true" /></summary>
    <div ref={panel} className={`${styles.panel} ${styles.effortPanel}`}>
      <div className={styles.effortHeading}>
        <SourceMenu providers={config?.providers || []} mutisEnabled={!!mutis.data?.enabled} value={settings.provider_id} title={name} disabled={streaming}
          open={sourceMenuOpen} setOpen={(open) => { setSourceMenuOpen(open); setModelMenuOpen(false); }} anchor={panel} onPick={changeProvider} />
        <strong>{chatText[settings.effort]}</strong>
        <button type="button" className={styles.resetEffort} disabled={streaming} aria-label={chatText.resetEffort} title={chatText.resetEffort} onClick={() => configure({ effort: "medium" })}><RotateCcw size={18} /></button>
      </div>
      <ModelMenu models={models} value={settings.model} label={modelLabel} disabled={streaming} open={modelMenuOpen}
        setOpen={open => { setModelMenuOpen(open); setSourceMenuOpen(false); }} anchor={panel}
        onPick={model => configure({ model })} />
      <div className={styles.effortSlider}>
        <input type="range" min={0} max={SLIDER_EFFORTS.length - 1} step={1} value={effortIndex} disabled={streaming} aria-label={chatText.effort} aria-valuetext={chatText[settings.effort]}
          style={{ background: `linear-gradient(to right, var(--effort-fill) ${effortIndex / (SLIDER_EFFORTS.length - 1) * 100}%, var(--color-muted) 0%)` }}
          onChange={event => configure({ effort: SLIDER_EFFORTS[Number(event.target.value)] })} />
        <div className={styles.effortTicks} aria-hidden="true">{SLIDER_EFFORTS.map((effort, index) => <span key={effort} data-filled={index < effortIndex} data-selected={index === effortIndex} />)}</div>
      </div>
    </div>
  </details>
  {provider && cli && <CliLimits key={provider.id} providerId={provider.id} name={provider.name} />}</>;
}
