import { useEffect, useRef, useState } from "react";
import { Menu, ChevronDown, RotateCcw } from "lucide-react";
import { useChatStore } from "@/components/features/chat/chat-store";
import { IconButton } from "@/components/icon-button/icon-button";
import { chatText } from "@/text/chat";
import { modelEffortOptions, effortsFor, effortFor } from "@/constants/chat";
import { SOURCE_LOGOS } from "@/constants/source-logos";
import { antigravityModel, ANTIGRAVITY_EFFORTS } from "@/lib/antigravity-models";
import { isModelVisible, useModelVisibility } from "@/lib/model-visibility";
import { modelDisplayLabel } from "@/lib/model-display-label";
import { usePopupFit } from "@/lib/use-popup-fit";
import type { Config } from "@/schema/config.schema";
import styles from "./chat-toolbar.module.css";
import { CliLimits } from "@/components/limits/cli-limits";
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
  const sourceMenu = useRef<HTMLDivElement>(null);
  const modelMenu = useRef<HTMLDivElement>(null);
  // Keep the panel mounted so the hook can observe it while the picker is closed.
  usePopupFit(picker, panel);
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      const target = event.target as Node;
      if (
        picker.current?.open &&
        !picker.current.contains(target) &&
        !sourceMenu.current?.contains(target) &&
        !modelMenu.current?.contains(target)
      ) {
        picker.current.open = false;
        setModelMenuOpen(false);
        setSourceMenuOpen(false);
      }
    }
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (!picker.current?.open) return;
      event.preventDefault();
      picker.current.open = false;
      setModelMenuOpen(false);
      setSourceMenuOpen(false);
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
  const lead = mutis.data?.lead;
  const worker = mutis.data?.worker;
  const checker = mutis.data?.delegate_checker ? mutis.data?.checker : undefined;
  const isMutis = settings.provider_id === "mutis";
  const provider = config?.providers.find((entry) => entry.id === (isMutis ? lead?.provider_id : settings.provider_id));
  const workerProvider = isMutis ? config?.providers.find((entry) => entry.id === worker?.provider_id) : undefined;
  const checkerProvider = isMutis ? config?.providers.find((entry) => entry.id === checker?.provider_id) : undefined;
  const models = provider ? [...new Set(provider.models)]
    .filter(model => model && isModelVisible(hiddenModels, provider.id, model)) : [];
  const cli = provider?.kind.endsWith("_cli");
  // Codex cannot write files on Windows until its sandbox is set up; check Lead plus MutiS Worker/Checker when they can write.
  const leadNeedsSandbox = provider?.kind === "codex_cli" && settings.permission !== "read";
  const workerNeedsSandbox = isMutis && workerProvider?.kind === "codex_cli" && worker?.permission !== "read";
  const checkerNeedsSandbox = isMutis && checkerProvider?.kind === "codex_cli" && checker?.permission !== "read";
  const codexSandbox = useQuery({ queryKey: ["codex-sandbox", provider?.id], enabled: leadNeedsSandbox,
    queryFn: ({ signal }) => codexApi.status(provider!.id, signal), staleTime: 30_000 });
  const workerCodexSandbox = useQuery({ queryKey: ["codex-sandbox", workerProvider?.id], enabled: workerNeedsSandbox,
    queryFn: ({ signal }) => codexApi.status(workerProvider!.id, signal), staleTime: 30_000 });
  const checkerCodexSandbox = useQuery({ queryKey: ["codex-sandbox", checkerProvider?.id], enabled: checkerNeedsSandbox,
    queryFn: ({ signal }) => codexApi.status(checkerProvider!.id, signal), staleTime: 30_000 });
  const needsSandboxSetup = (status?: { sandbox: string }) => Boolean(status && ["notConfigured", "updateRequired"].includes(status.sandbox));
  const chatOnly = !!provider && settings.provider_id !== "mutis" && !cli && !isAgentSource(provider);
  const permissionLimit = chatOnly ? { note: chatText.chatOnlyNote, chatOnly: true }
    : leadNeedsSandbox && needsSandboxSetup(codexSandbox.data) ? { note: chatText.sandboxNote }
    : workerNeedsSandbox && needsSandboxSetup(workerCodexSandbox.data) ? { note: chatText.workerSandboxNote }
    : checkerNeedsSandbox && needsSandboxSetup(checkerCodexSandbox.data) ? { note: chatText.checkerSandboxNote }
    : undefined;
  const permissionScope = isMutis ? { heading: chatText.mutisPermissionHeading, note: chatText.mutisPermissionScope } : undefined;
  useEffect(() => {
    if (!config) return;
    const first = config.providers[0];
    initialize(config.default_folder, first?.id || "", first?.models.find(model => isModelVisible(hiddenModels, first.id, model)) || "");
  }, [config, hiddenModels, initialize]);
  function changeProvider(id: string) {
    const next = config?.providers.find((entry) => entry.id === (id === "mutis" ? lead?.provider_id : id));
    const supportsCommands = next?.kind.endsWith("_cli") || isAgentSource(next);
    configure({ provider_id: id, model: next?.models.find(model => isModelVisible(hiddenModels, next.id, model)) || "", file_tools: "none",
      ...(settings.permission === "full" && !supportsCommands ? { permission: "write" as const } : {}) });
  }
  const sourceLogo = SOURCE_LOGOS[isMutis ? "mutis" : provider?.kind ?? ""];
  const isAntigravity = !isMutis && provider?.kind === "antigravity_cli";
  const variant = isAntigravity ? antigravityModel(settings.model) : undefined;
  const pickedModel = variant?.id ?? settings.model;
  const variants = isAntigravity ? models.map(id => ({ id, model: antigravityModel(id) })) : [];
  const variantModels = variants.filter(entry => entry.model.id === pickedModel);
  const mutisName = "MutiS";
  const name = isMutis ? mutisName : provider?.name || chatText.source;
  const modelLabel = isMutis ? "MutiS" : pickedModel || name;
  const summaryModelLabel = modelDisplayLabel(settings.model && !isMutis ? settings.model : modelLabel, settings.model && !isMutis ? provider?.kind : undefined);
  const catalogEffort = !isMutis && (provider?.kind === "codex_cli" || provider?.kind === "claude_cli")
    ? modelEffortOptions(provider.model_efforts?.[settings.model], settings.effort, provider.kind) : undefined;
  const efforts = isAntigravity
    ? (["default", ...ANTIGRAVITY_EFFORTS] as const).filter(level => variantModels.some(entry => (entry.model.effort ?? "default") === level))
    : catalogEffort?.choices ?? effortsFor(settings.model, provider?.kind).filter(effort => effort !== "default");
  const effort = isAntigravity ? "default" : catalogEffort?.selected ?? effortFor(settings.model, settings.effort, provider?.kind);
  const displayedEffort = isMutis ? effortFor(lead?.model ?? "", lead?.effort ?? "default", provider?.kind) : variant?.effort ?? effort;
  const resetEffort = isAntigravity ? efforts.find(level => level === "default" || level === "medium") ?? efforts[0] ?? "default"
    : catalogEffort?.reset ?? effortFor(settings.model, "medium", provider?.kind);
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
    if (provider && !isMutis && !streaming && effort !== settings.effort) configure({ effort });
  }, [provider, isMutis, effort, settings.effort, streaming, configure]);
  const mutisProviders = [...new Set([lead, mutis.data?.worker, mutis.data?.delegate_checker ? mutis.data.checker : undefined].map(role => role?.provider_id))]
    .flatMap(id => config?.providers.find(entry => entry.id === id) ?? []);
  const limitProviders = isMutis
    ? mutisProviders.filter(entry => entry.kind.endsWith("_cli"))
    : provider && cli ? [provider] : [];
  return <><ChatPermission supportsCommands={!!cli || isAgentSource(provider)} limit={permissionLimit} scope={permissionScope} menuRef={permissions} otherMenuRef={picker} />
  <div data-glass="model-group" className={styles.modelGroup}>
    <details ref={picker} data-glass="model-picker" className={styles.picker} onToggle={(event) => {
      if (event.target === event.currentTarget && event.currentTarget.open && permissions.current) permissions.current.open = false;
      if (event.target === event.currentTarget && !event.currentTarget.open) { setModelMenuOpen(false); setSourceMenuOpen(false); }
    }}>
      <summary>{sourceLogo && <img className={styles.sourceLogo} src={sourceLogo} width={16} height={16} alt="" />}
        <span>{summaryModelLabel}</span> <span className={styles.effort}>{effortLabel(displayedEffort)}</span><ChevronDown size={14} aria-hidden="true" /></summary>
      <div ref={panel} data-glass="overlay" className={`${styles.panel} ${styles.effortPanel}`}>
        <div className={styles.effortHeading}>
          <SourceMenu providers={config?.providers || []} mutisEnabled={!!mutis.data?.enabled} mutisName={mutisName} value={settings.provider_id} title={name} disabled={streaming}
            open={sourceMenuOpen} setOpen={(open) => { setSourceMenuOpen(open); setModelMenuOpen(false); }} anchor={panel} menuRef={sourceMenu} onPick={changeProvider} />
          <strong>{isMutis ? mutisName : effortLabel(displayedEffort)}</strong>
          {!isMutis && <button type="button" className={styles.resetEffort} disabled={streaming || ((isAntigravity || !!catalogEffort) && !efforts.length)} aria-label={`${chatText.effort}: ${effortLabel(resetEffort)}`} title={`${chatText.effort}: ${effortLabel(resetEffort)}`} onClick={() => pickEffort(resetEffort)}><RotateCcw size={18} /></button>}
        </div>
        {isMutis && mutisProviders.length > 0 && <p>{mutisProviders.map(entry => entry.name).join(" · ")}</p>}
        {!isMutis && efforts.length > 1 && <div className={styles.effortSlider}>
          <input type="range" min={0} max={efforts.length - 1} step={1} value={effortIndex} disabled={streaming} aria-label={chatText.effort} aria-valuetext={effortLabel(displayedEffort)}
            style={{ background: `linear-gradient(to right, var(--effort-fill) ${effortIndex / (efforts.length - 1) * 100}%, var(--color-muted) 0%)` }}
            onChange={event => pickEffort(efforts[Number(event.target.value)])} />
          <div className={styles.effortTicks} aria-hidden="true">{efforts.map((effort, index) => <span key={effort} data-filled={index < effortIndex} data-selected={index === effortIndex} />)}</div>
        </div>}
        {isMutis
          ? mutis.data && <MutisEfforts mutis={mutis.data} config={config} disabled={streaming} />
          : <ModelMenu models={isAntigravity ? [...new Set(variants.map(entry => entry.model.id))] : models} value={pickedModel} label={modelLabel} disabled={streaming} open={modelMenuOpen}
            setOpen={open => { setModelMenuOpen(open); setSourceMenuOpen(false); }} anchor={panel} menuRef={modelMenu}
            onPick={pickModel} />}
      </div>
    </details>
    {limitProviders.map(({ id, name, kind }) => (
      <CliLimits key={id} providerId={id} name={name} kind={kind} />
    ))}
  </div></>;
}
