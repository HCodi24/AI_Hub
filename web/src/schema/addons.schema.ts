import { z } from "zod";
const PluginSchema = z.object({
  id: z.string(), version: z.string().default(""), enabled: z.boolean().default(true), marketplace_source: z.string().default(""),
});
const McpServerSchema = z.object({ name: z.string(), summary: z.string().default("") });
const HomeSchema = z.object({
  path: z.string().default(""), plugins: z.array(PluginSchema).default([]), skills: z.array(z.string()).default([]),
  mcp: z.array(McpServerSchema).default([]),
});
export const AddonsSchema = z.object({ personal: HomeSchema, hub: HomeSchema, codex: HomeSchema });
export const AddonChangeResultSchema = z.object({ ok: z.boolean(), output: z.string().default("") });
export type Addons = z.infer<typeof AddonsSchema>;
export type AddonHome = z.infer<typeof HomeSchema>;
export type AddonKind = "plugin" | "skill" | "mcp";
export type AddonChange = { target: "claude" | "codex"; kind: AddonKind; name: string; action: "add" | "remove" };
