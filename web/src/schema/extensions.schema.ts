import { z } from "zod";
export const ExtensionActionSchema = z.object({ id: z.string(), label: z.string(), command: z.string(), modifies: z.string().default("") });
export const ExtensionSchema = z.object({
  id: z.string(), name: z.string(), description: z.string().default(""), homepage: z.string().default(""),
  installed: z.boolean().default(false), version: z.string().nullish(), codex: z.boolean().default(false),
  claude: z.boolean().default(false), folder_indexed: z.boolean().nullish(), actions: z.array(ExtensionActionSchema).default([]),
});
export const ExtensionsSchema = z.array(ExtensionSchema);
export type Extension = z.infer<typeof ExtensionSchema>;
export type ExtensionAction = z.infer<typeof ExtensionActionSchema>;
