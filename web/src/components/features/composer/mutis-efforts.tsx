import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { Config } from "@/schema/config.schema";
import type { Mutis } from "@/schema/mutis.schema";
import { mutisApi } from "@/apis/mutis.api";
import { effortFor, effortsFor } from "@/constants/chat";
import { chatText } from "@/text/chat";
import styles from "./chat-toolbar.module.css";

type Role = Mutis["roles"]["lead"];
const roleNames = ["lead", "small"] as const;

function RoleSlider({ role, provider, disabled, save }: {
  role: Role; provider?: Config["providers"][number]; disabled: boolean;
  save: (effort: Role["effort"]) => Promise<boolean>;
}) {
  const choices = effortsFor(role.model, provider?.kind);
  const selected = effortFor(role.model, role.effort, provider?.kind);
  const [draft, setDraft] = useState(selected);
  useEffect(() => setDraft(selected), [selected]);
  const index = Math.max(0, choices.indexOf(draft));
  const label = role.model || provider?.name || chatText.source;
  async function commit(value: string) {
    if (!await save(choices[Number(value)])) setDraft(selected);
  }
  return <div className={styles.modelEffort}>
    <div className={styles.modelEffortHeading}><span title={provider?.name}>{label}</span><strong>{chatText[draft]}</strong></div>
    <div className={styles.effortSlider}>
      <input type="range" min={0} max={choices.length - 1} step={1} value={index} disabled={disabled || !provider}
        aria-label={`${label}: ${chatText.effort}`} aria-valuetext={chatText[draft]}
        style={{ background: `linear-gradient(to right, var(--effort-fill) ${index / (choices.length - 1) * 100}%, var(--color-muted) 0%)` }}
        onChange={event => setDraft(choices[Number(event.target.value)])}
        onPointerUp={event => void commit(event.currentTarget.value)}
        onPointerCancel={() => setDraft(selected)}
        onKeyUp={event => { if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) void commit(event.currentTarget.value); }}
        onBlur={event => void commit(event.currentTarget.value)} />
      <div className={styles.effortTicks} aria-hidden="true">{choices.map((value, tick) => <span key={value} data-filled={tick < index} data-selected={tick === index} />)}</div>
    </div>
  </div>;
}

/** Persist model effort in the active MutiS profile, including shared lead/code roles. */
export function MutisEfforts({ mutis, config, disabled }: { mutis: Mutis; config?: Config; disabled: boolean }) {
  const client = useQueryClient();
  const profile = mutis.high_roles.lead.provider_id ? "high_roles" : "roles";
  const roles = mutis[profile];
  const activeRoleNames = roleNames;
  const unique = activeRoleNames.filter((name, index) => activeRoleNames.findIndex(other => roles[other].provider_id === roles[name].provider_id && roles[other].model === roles[name].model) === index);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const busy = useRef(false);
  async function save(role: Role, effort: Role["effort"]) {
    if (busy.current || disabled) return true;
    busy.current = true; setSaving(true); setError("");
    try {
      const current = await mutisApi.get();
      const next = { ...current[profile] };
      let changed = false;
      for (const name of activeRoleNames) {
        if (next[name].provider_id === role.provider_id && next[name].model === role.model && next[name].effort !== effort) {
          next[name] = { ...next[name], effort };
          changed = true;
        }
      }
      if (!changed) return true;
      const result = await mutisApi.put({ ...current, [profile]: next });
      client.setQueryData(["mutis"], result);
      return true;
    } catch {
      setError(chatText.requestError);
      void client.invalidateQueries({ queryKey: ["mutis"] });
      return false;
    } finally { busy.current = false; setSaving(false); }
  }
  return <div className={styles.modelEfforts} aria-busy={saving}>
    {unique.map(name => <RoleSlider key={`${profile}:${name}:${roles[name].provider_id}:${roles[name].model}`} role={roles[name]}
      provider={config?.providers.find(provider => provider.id === roles[name].provider_id)} disabled={disabled || saving}
      save={effort => save(roles[name], effort)} />)}
    {error && <p role="alert">{error}</p>}
  </div>;
}
