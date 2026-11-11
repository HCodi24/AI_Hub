import { z } from "zod";
export const ProviderSummarySchema = z.object({
  id: z.string(), name: z.string(), kind: z.string(), models: z.array(z.string()).default([]),
  agent_capable: z.boolean().default(false),
});
export const ConfigSchema = z.object({ providers: z.array(ProviderSummarySchema), default_folder: z.string() });
export type ProviderSummary = z.infer<typeof ProviderSummarySchema>;
export type Config = z.infer<typeof ConfigSchema>;
