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
const LinkSkillSchema = z.object({ id: z.string(), name: z.string(), path: z.string() });
export const LinkFoundSchema = z.object({
  source: z.string(), claude: LinkMarketSchema, codex: LinkMarketSchema, skills: z.array(LinkSkillSchema).default([]),
});
const LinkTargetSchema = z.enum(["claude", "codex"]);
const LinkSkillConflictSchema = z.object({
  skill_id: z.string(),
  target: LinkTargetSchema,
  confirmation: z.string(),
});
export const LinkSkillInstallResultSchema = AddonChangeResultSchema.extend({
  conflict: LinkSkillConflictSchema.optional(),
});
const CodexPluginSchema = z.object({ id: z.string(), version: z.string().default(""), enabled: z.boolean().default(true) });
/** `plugins` are in MutiS's Codex; `available` only in the user's own Codex. */
export const CodexPluginsSchema = z.object({ plugins: z.array(CodexPluginSchema), available: z.array(CodexPluginSchema).default([]) });
export type LinkFound = z.infer<typeof LinkFoundSchema>;
export type LinkTarget = z.infer<typeof LinkTargetSchema>;
export type LinkSkillConflict = z.infer<typeof LinkSkillConflictSchema>;
export type LinkSkillInstall = { link: string; skill_id: string; target: LinkTarget; confirmation?: string };
