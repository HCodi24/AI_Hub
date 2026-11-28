import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { Config } from "@/schema/config.schema";
import type { Mutis } from "@/schema/mutis.schema";
import { mutisApi } from "@/apis/mutis.api";
import { roleEffortOptions } from "@/constants/chat";
import { modelDisplayLabel } from "@/lib/model-display-label";
import { chatText } from "@/text/chat";
import styles from "./chat-toolbar.module.css";

type Role = Mutis["lead"];
const roleNames = ["lead", "worker"] as const;

function sameModel(left: Role, right: Role) {
  return left.provider_id === right.provider_id && left.model === right.model;
}

function RoleSlider({ role, provider, disabled, save }: {
  role: Role; provider?: Config["providers"][number]; disabled: boolean;
  save: (effort: Role["effort"]) => Promise<boolean>;
}) {
  const { choices, selected } = roleEffortOptions(provider, role.model, role.effort);
  const [draft, setDraft] = useState(selected);
  useEffect(() => setDraft(selected), [selected]);
  const displayedEffort = choices.includes(draft) ? draft : selected;
  const index = choices.indexOf(displayedEffort);
  const lastIndex = choices.length - 1;
  const label = role.model ? modelDisplayLabel(role.model, provider?.kind) : provider?.name || chatText.source;
  const providerName = provider?.name;
  const title = providerName && role.model && providerName !== role.model
    ? `${providerName} · ${role.model}`
    : providerName || role.model || undefined;
  async function commit(value: string) {
    const effort = choices[Number(value)];
    if (effort && !await save(effort)) setDraft(selected);
  }
  return <div className={styles.modelEffort}>
    <div className={styles.modelEffortHeading}><span title={title}>{label}</span><strong>{chatText[displayedEffort]}</strong></div>
    <div className={styles.effortSlider}>
      <input type="range" min={0} max={lastIndex} step={1} value={index} disabled={disabled || !provider}
        aria-label={`${label}: ${chatText.effort}`} aria-valuetext={chatText[displayedEffort]}
        style={{ background: `linear-gradient(to right, var(--effort-fill) ${index / Math.max(1, lastIndex) * 100}%, var(--color-muted) 0%)` }}
        onChange={event => setDraft(choices[Number(event.target.value)])}
        onPointerUp={event => void commit(event.currentTarget.value)}
        onPointerCancel={() => setDraft(selected)}
        onKeyUp={event => { if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) void commit(event.currentTarget.value); }}
        onBlur={event => void commit(event.currentTarget.value)} />
      <div className={styles.effortTicks} aria-hidden="true">{choices.map((value, tick) => <span key={value} data-filled={tick < index} data-selected={tick === index} />)}</div>
    </div>
  </div>;
}

export function MutisEfforts({ mutis, config, disabled }: { mutis: Mutis; config?: Config; disabled: boolean }) {
  const client = useQueryClient();
  const unique = sameModel(mutis.lead, mutis.worker) ? roleNames.slice(0, 1) : roleNames;
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const busy = useRef(false);
  async function save(role: Role, effort: Role["effort"]) {
    if (busy.current || disabled) return true;
    busy.current = true; setSaving(true); setError("");
    try {
      const current = await mutisApi.get();
      const next = { ...current, lead: { ...current.lead }, worker: { ...current.worker } };
      let changed = false;
      for (const name of roleNames) {
        if (sameModel(current[name], role) && current[name].effort !== effort) {
          next[name].effort = effort;
          changed = true;
        }
      }
      if (!changed) return true;
      const result = await mutisApi.put(next);
      client.setQueryData(["mutis"], result);
      return true;
    } catch {
      setError(chatText.requestError);
      void client.invalidateQueries({ queryKey: ["mutis"] });
      return false;
    } finally { busy.current = false; setSaving(false); }
  }
  return <div className={styles.modelEfforts} aria-busy={saving}>
    {unique.map(name => <RoleSlider key={`${name}:${mutis[name].provider_id}:${mutis[name].model}`} role={mutis[name]}
      provider={config?.providers.find(provider => provider.id === mutis[name].provider_id)} disabled={disabled || saving}
      save={effort => save(mutis[name], effort)} />)}
    {error && <p role="alert">{error}</p>}
  </div>;
}
