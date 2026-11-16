import { useId } from "react";
import { z } from "zod";
import { useProviderConnections } from "@/lib/provider-connections";
import { PermissionSchema, EffortSchema } from "@/schema/chat.schema";
import type { ProviderSummary } from "@/schema/config.schema";
import type { Mutis } from "@/schema/mutis.schema";
import { effortsFor, effortFor, PERMISSIONS } from "@/constants/chat";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
type Value = { assignee: string; model?: string | null; effort: z.infer<typeof EffortSchema>; permission?: z.infer<typeof PermissionSchema> | null };
export function SourceControls({ providers, roles, value, change, disabled, showPermission = true }: { providers: ProviderSummary[]; roles?: Mutis["roles"]; value: Value; change: (value: Value) => void; disabled?: boolean; showPermission?: boolean }) {
  const modelsId = useId();
  const statuses = useProviderConnections(providers);
  const provider = providers.find(p => p.id === value.assignee);
  return <div><label>{t.source}<select value={value.assignee} disabled={disabled} onChange={event => {
    const id = event.target.value; const role = id.startsWith("role:") ? roles?.[id.slice(5) as keyof typeof roles] : undefined;
    const next = providers.find(p => p.id === (role?.provider_id || id));
    change({ ...value, assignee: next?.id || id, model: role?.model || next?.models[0] || "", effort: role?.effort || "default", permission: "write" });
  }}><option value="">{t.source}</option>{providers.map(p => <option key={p.id} value={p.id} disabled={statuses[p.id]?.ok === false}>{p.name}{statuses[p.id]?.ok === false ? ` · ${t.missingSource}` : ""}</option>)}
    {roles && Object.entries(roles).map(([key, role]) => <option key={key} value={`role:${key}`} disabled={statuses[role.provider_id]?.ok === false}>MutiS · {key}</option>)}</select></label>
    <label>{t.model}<input list={modelsId} key={`${value.assignee}:${value.model}`} defaultValue={value.model || ""} disabled={disabled} onBlur={event => { if (event.target.value !== value.model) change({ ...value, model: event.target.value }); }} /><datalist id={modelsId}>{provider?.models.map(model => <option key={model} value={model} />)}</datalist></label>
    <label>{t.effort}<select value={effortFor(value.model || "", value.effort, provider?.kind)} disabled={disabled} onChange={event => change({ ...value, effort: EffortSchema.parse(event.target.value) })}>{effortsFor(value.model || "", provider?.kind).map(e => <option key={e} value={e}>{chatText[e]}</option>)}</select></label>
    {showPermission && <label>{t.permission}<select value={value.permission || "write"} disabled={disabled} onChange={event => change({ ...value, permission: PermissionSchema.parse(event.target.value) })}>{PERMISSIONS.map(e => <option key={e} value={e} disabled={e === "full" && !provider?.kind.endsWith("_cli")}>{chatText[e]}</option>)}</select></label>}
    {showPermission && provider && !provider.kind.endsWith("_cli") && <small>{t.noCommands}</small>}
  </div>;
}
