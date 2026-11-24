import { devApi } from "@/apis/dev.api";
import { mutisApi } from "@/apis/mutis.api";
import { useChatStore } from "@/components/features/chat/chat-store";
import { notify } from "@/components/toast/toast";
import type { ProviderSummary } from "@/schema/config.schema";
import { settingsText as t } from "@/text/settings";
/** Start a "Chat phát triển": a new chat on the AI Hub repo itself. The server adds the dev preamble to the first message. */
export async function openDevChat(providers: ProviderSummary[]) {
  const store = useChatStore.getState();
  if (store.streaming) return;
  let path = "";
  try { path = (await devApi.repo()).path; } catch { /* reported below */ }
  if (!path) { notify(t.dev.noRepo); return; }
  const mutis = await mutisApi.get().catch(() => null);
  const codex = providers.find(provider => provider.kind === "codex_cli");
  store.reset();
  // MutiS uses its configured lead. Standalone Codex CLI keeps the development chat preset.
  store.configure({
    dev_chat: true, folder: path, permission: "write", file_tools: "none",
    ...(mutis?.enabled ? { provider_id: "mutis", model: "", effort: "default" as const }
      : codex ? { provider_id: codex.id, model: codex.models[0] || "", effort: "high" as const } : {}),
  });
}
