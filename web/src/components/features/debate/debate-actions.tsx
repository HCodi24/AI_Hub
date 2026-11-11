import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { debatesApi } from "@/apis/debates.api";
import { configApi } from "@/apis/config.api";
import { useChat } from "@/components/features/chat/use-chat";
import { useChatStore } from "@/components/features/chat/chat-store";
import { useCardStore } from "@/components/features/chat/card-store";
import { startCard } from "@/components/features/chat/card-actions";
import { SourceControls } from "@/components/features/plan/source-controls";
import { notify } from "@/components/toast/toast";
import { cardText as t } from "@/text/cards";
import type { Debate } from "@/schema/debates.schema";
export function DebateActions({ debate }: { debate: Debate }) {
  const [open, setOpen] = useState(false); const [busy, setBusy] = useState(false);
  const { send } = useChat();
  const { streaming, settings, configure } = useChatStore(); const running = useCardStore(s => s.running);
  const config = useQuery({ queryKey: ["config"], queryFn: () => configApi.get() });
  const blocked = busy || streaming || Boolean(running);
  async function act(action: () => Promise<unknown>) { setBusy(true); try { await action(); } catch (e) { notify(e instanceof Error ? e.message : t.error); } finally { setBusy(false); } }
  return <div><button disabled={blocked || Boolean(debate.plan_id)} onClick={() => void act(async () => {
    const full = await debatesApi.get(debate.id); await startCard(full.plan_request, "plan", debate.id);
  })}>{t.planExecute}</button>
    <button disabled={blocked} aria-expanded={open} onClick={() => setOpen(!open)}>{t.executeWith}</button>
    {open && <div role="group" aria-label={t.executeWith}>
      <SourceControls providers={config.data?.providers || []} value={{ assignee: settings.provider_id, model: settings.model, effort: settings.effort, permission: settings.permission }}
        change={value => configure({ provider_id: value.assignee, model: value.model || "", effort: value.effort, permission: value.permission || "write" })} />
      <button disabled={blocked || !config.data?.providers.some(p => p.id === settings.provider_id)} onClick={() => void act(async () => {
        const full = await debatesApi.get(debate.id);
        const provider = config.data?.providers.find(item => item.id === settings.provider_id);
        configure({ folder: full.folder, dev_chat: false, file_tools: provider?.agent_capable ? settings.permission === "read" ? "read" : "write" : "none" });
        setOpen(false);
        await send(`${t.directInstruction}\n\n${full.plan_request}`, true);
      })}>{t.execute}</button></div>}
  </div>;
}
