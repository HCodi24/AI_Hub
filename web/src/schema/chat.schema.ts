import { z } from "zod";
export const PermissionSchema = z.enum(["read", "write", "full"]);
export const EffortSchema = z.enum(["default", "low", "medium", "high", "xhigh", "max"]);
export const FileToolsSchema = z.enum(["none", "read", "write"]);
export const FileChangeSchema = z.object({ path: z.string(), added: z.number(), removed: z.number() });
export const ChatMessageSchema = z.object({
  role: z.enum(["user", "assistant", "system"]), content: z.string(),
  reasoning: z.string().default(""), logs: z.array(z.string()).default([]), diff: z.string().default(""), files: z.array(FileChangeSchema).default([]),
  error: z.string().nullish(), debate_id: z.string().nullish(), plan_id: z.string().nullish(), provider: z.string().default(""),
  attachments: z.array(z.object({ id: z.string(), mime: z.string(), path: z.string().default("") })).default([]),
});
export const ChatSummarySchema = z.object({ id: z.string().uuid(), title: z.string(), updated_at: z.number(), dev_chat: z.boolean().default(false), folder: z.string().default("") });
export const ChatRecordSchema = ChatSummarySchema.extend({
  provider_id: z.string(), model: z.string(), folder: z.string(),
  permission: PermissionSchema.default("write"), effort: EffortSchema.default("default"),
  file_tools: FileToolsSchema.default("none"), messages: z.array(ChatMessageSchema),
});
export const ChatRequestSchema = z.object({
  chat_id: z.string().uuid(), provider_id: z.string().min(1), model: z.string().min(1), folder: z.string(),
  permission: PermissionSchema, effort: EffortSchema, file_tools: FileToolsSchema,
  dev_chat: z.boolean(), messages: z.array(ChatMessageSchema),
});
export type ChatMessage = z.infer<typeof ChatMessageSchema>;
export type ChatRecord = z.infer<typeof ChatRecordSchema>;
export type ChatRequest = z.infer<typeof ChatRequestSchema>;
export type ChatSummary = z.infer<typeof ChatSummarySchema>;
export const StreamDataSchema = z.object({
  text: z.string().optional(), message: z.string().optional(), provider_name: z.string().optional(), provider_kind: z.string().optional(), model: z.string().optional(),
  effort: z.string().optional(), role: z.string().nullish(), elapsed_s: z.number().optional(),
  idle_s: z.number().optional(), last_activity: z.string().optional(), scope: z.string().optional(),
  action: z.string().optional(), status: z.string().optional(), files: z.array(FileChangeSchema).optional(),
});
export type StreamData = z.infer<typeof StreamDataSchema>;
