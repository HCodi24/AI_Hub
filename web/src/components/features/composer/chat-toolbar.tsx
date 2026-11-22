import { useEffect, useRef, useState } from "react";
import { Menu, ChevronDown, RotateCcw } from "lucide-react";
import { useChatStore } from "@/components/features/chat/chat-store";
import { IconButton } from "@/components/icon-button/icon-button";
import { chatText } from "@/text/chat";
import { modelEffortOptions, effortsFor, effortFor } from "@/constants/chat";
import { SOURCE_LOGOS } from "@/constants/source-logos";
import { antigravityModel, ANTIGRAVITY_EFFORTS } from "@/lib/antigravity-models";
import { rolesFor } from "@/lib/mutis-level";
import { isModelVisible, useModelVisibility } from "@/lib/model-visibility";
import type { Config } from "@/schema/config.schema";
import styles from "./chat-toolbar.module.css";
import { CliLimits } from "./cli-limits";
import { useQuery } from "@tanstack/react-query";
import { SourceMenu } from "./source-menu";
import { ModelMenu } from "./model-menu";
import { isAgentSource } from "@/lib/agent-source";
import { codexApi } from "@/apis/codex.api";
import { mutisApi } from "@/apis/mutis.api";
import { ChatPermission } from "./chat-permission";
import { useShallow } from "zustand/react/shallow";
import { MutisEfforts } from "./mutis-efforts";

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
  const hiddenModels = useModelVisibility(state => state.hiddenModels);
  const picker = useRef<HTMLDetailsElement>(null);
  const permissions = useRef<HTMLDetailsElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      if (picker.current?.open && !picker.current.contains(event.target as Node)) {
        picker.current.open = false;
        setModelMenuOpen(false);
      }
    }
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented) return;
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
  const activeRoles = rolesFor(mutis.data, settings.effort);
  const provider = config?.providers.find((entry) => entry.id === (settings.provider_id === "mutis" ? activeRoles?.lead.provider_id : settings.provider_id));
  const models = provider ? [...new Set(provider.models)]
    .filter(model => model && isModelVisible(hiddenModels, provider.id, model)) : [];
  const cli = provider?.kind.endsWith("_cli");
  // Codex cannot write files on Windows until its sandbox is set up; ask only when the chat could write.
  const codexSandbox = useQuery({ queryKey: ["codex-sandbox", provider?.id], enabled: provider?.kind === "codex_cli" && settings.permission !== "read",
    queryFn: ({ signal }) => codexApi.status(provider!.id, signal), staleTime: 30_000 });
  const chatOnly = !!provider && settings.provider_id !== "mutis" && !cli && !isAgentSource(provider);
  const permissionLimit = chatOnly ? { note: chatText.chatOnlyNote, chatOnly: true }
    : codexSandbox.data && ["notConfigured", "updateRequired"].includes(codexSandbox.data.sandbox) ? { note: chatText.sandboxNote } : undefined;
  useEffect(() => {
    if (!config) return;
    const first = config.providers[0];
    initialize(config.default_folder, first?.id || "", first?.models.find(model => isModelVisible(hiddenModels, first.id, model)) || "");
  }, [config, hiddenModels, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === (id === "mutis" ? activeRoles?.lead.provider_id : id));
    const supportsCommands = next?.kind.endsWith("_cli") || isAgentSource(next);
    configure({ provider_id: id, model: next?.models.find(model => isModelVisible(hiddenModels, next.id, model)) || "", file_tools: "none",
      ...(settings.permission === "full" && !supportsCommands ? { permission: "write" as const } : {}) });
  }
  // MutiS has a single level (High); the models come from Settings.
  const isMutis = settings.provider_id === "mutis";
  const sourceLogo = SOURCE_LOGOS[isMutis ? "mutis" : provider?.kind ?? ""];
  const isAntigravity = !isMutis && provider?.kind === "antigravity_cli";
  const variant = isAntigravity ? antigravityModel(settings.model) : undefined;
  const pickedModel = variant?.id ?? settings.model;
  const variants = isAntigravity ? models.map(id => ({ id, model: antigravityModel(id) })) : [];
  const variantModels = variants.filter(entry => entry.model.id === pickedModel);
  const mutisName = "MutiS";
  const name = isMutis ? mutisName : provider?.name || chatText.source;
  const modelLabel = isMutis ? "MutiS" : pickedModel || name;
  const gemini = isAntigravity ? /^gemini-(\d+(?:\.\d+)*)-([a-z]+)$/i.exec(pickedModel) : null;
  const summaryModelLabel = gemini
    ? `${gemini[2][0].toUpperCase()}${gemini[2].slice(1)}-${gemini[1]}`
    : provider?.kind === "codex_cli" && !isMutis
      ? modelLabel.replace(/^gpt-(\d+(?:\.\d+)*)-([a-z]+)$/i, (_, version: string, family: string) => `${family[0].toUpperCase()}${family.slice(1)}-${version}`)
      : modelLabel.replace(/^claude-([a-z]+)-(\d+(?:-\d{1,2})*)$/i, (_, family: string, version: string) =>
        `${family[0].toUpperCase()}${family.slice(1)}-${version.replaceAll("-", ".")}`);
  const catalogEffort = !isMutis && (provider?.kind === "codex_cli" || provider?.kind === "claude_cli")
    ? modelEffortOptions(provider.model_efforts?.[settings.model], settings.effort, provider.kind) : undefined;
  const efforts = isAntigravity
    ? (["default", ...ANTIGRAVITY_EFFORTS] as const).filter(level => variantModels.some(entry => (entry.model.effort ?? "default") === level))
    : catalogEffort?.choices ?? effortsFor(settings.model, provider?.kind).filter(effort => effort !== "default");
  const effort = isMutis ? "high" : isAntigravity ? "default" : catalogEffort?.selected ?? effortFor(settings.model, settings.effort, provider?.kind);
  const displayedEffort = variant?.effort ?? effort;
  const resetEffort = isAntigravity ? efforts.find(level => level === "default" || level === "medium") ?? efforts[0] ?? "default"
    : isMutis ? "high" : catalogEffort?.reset ?? effortFor(settings.model, "medium", provider?.kind);
  const effortIndex = Math.max(0, efforts.findIndex(choice => choice === (!isAntigravity && !catalogEffort && effort === "default" ? "medium" : displayedEffort)));
  const effortLabel = (level: typeof displayedEffort) => level === "thinking" ? chatText.petStatus.thinking : chatText[level];
  function pickEffort(level: typeof displayedEffort) {
    if (isAntigravity) {
      const next = variantModels.find(entry => (entry.model.effort ?? "default") === level);
      if (next) configure({ model: next.id, effort: "default" });
    } else if (level !== "thinking") configure({ effort: level });
  }
  function pickModel(model: string) {
    if (isAntigravity) {
      const available = variants.filter(entry => entry.model.id === model);
      const next = available.find(entry => entry.model.effort === variant?.effort) ?? available[0];
      if (next) configure({ model: next.id, effort: "default" });
    } else configure({ model });
  }
  useEffect(() => {
    if ((provider || isMutis) && !streaming && effort !== settings.effort) configure({ effort });
  }, [provider, isMutis, effort, settings.effort, streaming, configure]);
  // Limits of every CLI source that takes part: the lead, and under MutiS also the workers (and the checker at High), e.g. Claude and Codex together.
  const limitProviders = isMutis
    ? [...new Set([activeRoles?.lead, activeRoles?.big, activeRoles?.small, ...(mutis.data?.delegate_checker ? [mutis.data.high_checker] : [])].map(role => role?.provider_id))]
      .flatMap(id => config?.providers.find(entry => entry.id === id && entry.kind.endsWith("_cli")) ?? [])
    : provider && cli ? [provider] : [];
  return <><ChatPermission supportsCommands={!!cli || isAgentSource(provider)} limit={permissionLimit} menuRef={permissions} otherMenuRef={picker} />
  <details ref={picker} className={styles.picker} onToggle={(event) => {
    if (event.target === event.currentTarget && event.currentTarget.open && permissions.current) permissions.current.open = false;
    if (event.target === event.currentTarget && !event.currentTarget.open) { setModelMenuOpen(false); setSourceMenuOpen(false); }
  }}>
    <summary>{sourceLogo && <img className={styles.sourceLogo} src={sourceLogo} width={16} height={16} alt="" />}
      <span>{summaryModelLabel}</span> <span className={styles.effort}>{effortLabel(displayedEffort)}</span><ChevronDown size={14} aria-hidden="true" /></summary>
    <div ref={panel} className={`${styles.panel} ${styles.effortPanel}`}>
      <div className={styles.effortHeading}>
        <SourceMenu providers={config?.providers || []} mutisEnabled={!!mutis.data?.enabled} mutisName={mutisName} value={settings.provider_id} title={name} disabled={streaming}
          open={sourceMenuOpen} setOpen={(open) => { setSourceMenuOpen(open); setModelMenuOpen(false); }} anchor={panel} onPick={changeProvider} />
        <strong>{isMutis ? mutisName : effortLabel(displayedEffort)}</strong>
        <button type="button" className={styles.resetEffort} disabled={streaming || ((isAntigravity || !!catalogEffort) && !efforts.length)} aria-label={`${chatText.effort}: ${effortLabel(resetEffort)}`} title={`${chatText.effort}: ${effortLabel(resetEffort)}`} onClick={() => pickEffort(resetEffort)}><RotateCcw size={18} /></button>
      </div>
      {!isMutis && efforts.length > 1 && <div className={styles.effortSlider}>
        <input type="range" min={0} max={efforts.length - 1} step={1} value={effortIndex} disabled={streaming} aria-label={chatText.effort} aria-valuetext={effortLabel(displayedEffort)}
          style={{ background: `linear-gradient(to right, var(--effort-fill) ${effortIndex / (efforts.length - 1) * 100}%, var(--color-muted) 0%)` }}
          onChange={event => pickEffort(efforts[Number(event.target.value)])} />
        <div className={styles.effortTicks} aria-hidden="true">{efforts.map((effort, index) => <span key={effort} data-filled={index < effortIndex} data-selected={index === effortIndex} />)}</div>
      </div>}
      {isMutis
        ? mutis.data && <MutisEfforts mutis={mutis.data} config={config} disabled={streaming} />
        : <ModelMenu models={isAntigravity ? [...new Set(variants.map(entry => entry.model.id))] : models} value={pickedModel} label={modelLabel} disabled={streaming} open={modelMenuOpen}
          setOpen={open => { setModelMenuOpen(open); setSourceMenuOpen(false); }} anchor={panel}
          onPick={pickModel} />}
    </div>
  </details>
  {limitProviders.map(entry => <CliLimits key={entry.id} providerId={entry.id} name={entry.name} />)}</>;
}
