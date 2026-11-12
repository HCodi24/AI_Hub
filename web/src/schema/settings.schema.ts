import { z } from "zod";
export const ProviderSchema = z.object({
  id: z.string(), name: z.string(), kind: z.string(), base_url: z.string().default(""), api_key: z.string().default(""),
  executable: z.string().default(""), models: z.array(z.string()).default([]), agent_capable: z.boolean().default(false),
});
export const MemoryConfigSchema = z.object({
  enabled: z.boolean().default(true), max_scopes_in_prompt: z.number().default(3), max_chars_per_scope: z.number().default(6000),
});
export const McpServerSchema = z.object({
  name: z.string(), command: z.string(), args: z.array(z.string()).default([]),
  env: z.record(z.string(), z.string()).default({}), enabled: z.boolean().default(true),
});
export const McpToolsSchema = z.object({ tools: z.array(z.object({ name: z.string(), description: z.string().default(""), read_only: z.boolean().default(false) })) });
export const FullConfigSchema = z.object({
  providers: z.array(ProviderSchema), default_folder: z.string().default(""),
  chat_idle_timeout_minutes: z.number().default(10), planner_provider_id: z.string().default(""),
  planner_model: z.string().default(""), memory: MemoryConfigSchema.default({}), mcp_servers: z.array(McpServerSchema).default([]), pets: z.record(z.string(), z.string()).default({}),
}).passthrough();
export const ConnectionTestSchema = z.object({
  ok: z.boolean(), status: z.string(), message: z.string().default(""), detail: z.string().default(""),
});
const CliInfoSchema = z.object({ installed: z.boolean(), version: z.string().nullish(), path: z.string().nullish() });
export const CliStatusSchema = z.object({ codex_cli: CliInfoSchema, claude_cli: CliInfoSchema });
export const CliCandidatesSchema = z.array(z.object({ path: z.string(), version: z.string(), source: z.string() }));
export const ModelsSchema = z.object({ models: z.array(z.string()) });
export const LoginStartSchema = z.object({ session_id: z.string(), url: z.string() });
export const LoginResultSchema = z.object({
  logged_in: z.boolean(), message: z.string().default(""), output_tail: z.string().default(""),
});
export type Provider = z.infer<typeof ProviderSchema>;
export type FullConfig = z.infer<typeof FullConfigSchema>;
export type McpServer = z.infer<typeof McpServerSchema>;
export type MemoryConfig = z.infer<typeof MemoryConfigSchema>;
export type ConnectionTest = z.infer<typeof ConnectionTestSchema>;
export type ProviderInput = Omit<Provider, "id">;
