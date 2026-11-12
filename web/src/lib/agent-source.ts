import type { ProviderSummary } from "@/schema/config.schema";
/** API-key sources that run MutiS's own agent (read/edit files, run commands with full permission, MCP). CLIs do this themselves. */
export const isAgentSource = (provider?: Pick<ProviderSummary, "kind" | "agent_capable">) =>
  provider?.kind === "anthropic" || (provider?.kind === "openai_compat" && provider.agent_capable);
/** File tools follow the chat permission: read-only when read, otherwise write (full permission also lets the agent run commands). */
export const fileToolsFor = (provider: Pick<ProviderSummary, "kind" | "agent_capable"> | undefined, permission: string) =>
  !isAgentSource(provider) ? "none" as const : permission === "read" ? "read" as const : "write" as const;
