import { z } from "zod";
export const DevStatusSchema = z.object({ dev: z.boolean(), version: z.string().optional() });
export const DevRepoSchema = z.object({
  path: z.string(), branch: z.string().default(""), changed_files: z.number().default(0),
  last_commit: z.object({ hash: z.string().default(""), subject: z.string().default(""), author: z.string().default("") }),
});
export const DevPreviewServerSchema = z.object({ port: z.number(), running: z.boolean(), owned: z.boolean() });
export type DevRepo = z.infer<typeof DevRepoSchema>;
