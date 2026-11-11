import { z } from "zod";
export const DevStatusSchema = z.object({ dev: z.boolean() });
export const DevRepoSchema = z.object({
  path: z.string(), branch: z.string().default(""), changed_files: z.number().default(0),
  last_commit: z.object({ hash: z.string().default(""), subject: z.string().default(""), author: z.string().default("") }),
});
export const DevDiffSchema = z.object({
  stat: z.string().default(""), diff: z.string().default(""), untracked: z.array(z.string()).default([]),
  truncated: z.boolean().default(false),
});
export const DevGitResultSchema = z.object({
  ok: z.boolean().default(true), skipped: z.boolean().default(false), message: z.string().default(""), hash: z.string().default(""),
}).passthrough();
export const DevSettingsSchema = z.object({ auto_rebuild: z.boolean().default(false) });
export type DevRepo = z.infer<typeof DevRepoSchema>;
export type DevDiff = z.infer<typeof DevDiffSchema>;
