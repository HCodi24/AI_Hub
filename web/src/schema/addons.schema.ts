import { z } from "zod";
const PluginSchema = z.object({
  id: z.string(), version: z.string().default(""), enabled: z.boolean().default(true), marketplace_source: z.string().default(""),
});
const McpServerSchema = z.object({ name: z.string(), summary: z.string().default("") });
const HomeSchema = z.object({
  path: z.string().default(""), plugins: z.array(PluginSchema).default([]), skills: z.array(z.string()).default([]),
  mcp: z.array(McpServerSchema).default([]),
});
export const AddonsSchema = z.object({ personal: HomeSchema, hub: HomeSchema });
export const AddonChangeResultSchema = z.object({ ok: z.boolean(), output: z.string().default("") });
export type Addons = z.infer<typeof AddonsSchema>;
export type AddonHome = z.infer<typeof HomeSchema>;
export type AddonKind = "plugin" | "skill" | "mcp";
export type AddonChange = { target: "claude" | "codex"; kind: AddonKind; name: string; action: "add" | "remove" };
const LinkMarketSchema = z.object({ name: z.string(), plugins: z.array(z.object({ name: z.string(), description: z.string().default("") })) }).nullable();
/** What a GitHub link can install: one marketplace per CLI that understands it. */
export const LinkFoundSchema = z.object({ source: z.string(), claude: LinkMarketSchema, codex: LinkMarketSchema });
const CodexPluginSchema = z.object({ id: z.string(), version: z.string().default(""), enabled: z.boolean().default(true) });
/** `plugins` are in MutiS's Codex; `available` only in the user's own Codex. */
export const CodexPluginsSchema = z.object({ plugins: z.array(CodexPluginSchema), available: z.array(CodexPluginSchema).default([]) });
export type LinkFound = z.infer<typeof LinkFoundSchema>;
export type LinkTarget = "claude" | "codex";
