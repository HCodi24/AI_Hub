import { z } from "zod";
export const PermissionSchema = z.enum(["read", "write", "full"]);
export const EffortSchema = z.enum(["default", "low", "medium", "high", "xhigh", "max"]);
export const FileToolsSchema = z.enum(["none", "read", "write"]);
export const FileChangeSchema = z.object({ path: z.string(), status: z.enum(["added", "modified", "deleted"]).optional(), added: z.number(), removed: z.number() });
export const TaskRowSchema = z.object({
  title: z.string(), state: z.string(), attempt: z.number().default(0), check: z.string().nullish(),
  actor: z.string().nullish(), model: z.string().nullish(),
  checker: z.string().nullish(), checker_model: z.string().nullish(),
});
export const ChatMessageSchema = z.object({
  role: z.enum(["user", "assistant", "system"]), content: z.string(),
  reasoning: z.string().default(""), logs: z.array(z.string()).default([]), diff: z.string().default(""), files: z.array(FileChangeSchema).default([]),
  error: z.string().nullish(), debate_id: z.string().nullish(), plan_id: z.string().nullish(), provider: z.string().default(""),
  model: z.string().default(""),
  tasks: z.array(TaskRowSchema).optional(),
  attachments: z.array(z.object({ id: z.string(), mime: z.string(), path: z.string().default("") })).default([]),
});
export const ChatSummarySchema = z.object({ id: z.string().uuid(), title: z.string(), updated_at: z.number(), dev_chat: z.boolean().default(false), folder: z.string().default("") });
export const ChatRecordSchema = ChatSummarySchema.extend({
  provider_id: z.string(), model: z.string(), folder: z.string(),
  permission: PermissionSchema.default("write"), effort: EffortSchema.default("default"),
  /** Set when the chat ran at the MutiS source: the effort the user picked there. Absent = a single model. */
  mutis_level: EffortSchema.nullish().catch(null),
  file_tools: FileToolsSchema.default("none"), messages: z.array(ChatMessageSchema),
});
export const ChatRequestSchema = z.object({
  chat_id: z.string().uuid(), provider_id: z.string().min(1), model: z.string(), folder: z.string(),
  permission: PermissionSchema, effort: EffortSchema, file_tools: FileToolsSchema,
  dev_chat: z.boolean(), messages: z.array(ChatMessageSchema),
});
export type ChatMessage = z.infer<typeof ChatMessageSchema>;
export type ChatRecord = z.infer<typeof ChatRecordSchema>;
export type ChatRequest = z.infer<typeof ChatRequestSchema>;
export type ChatSummary = z.infer<typeof ChatSummarySchema>;
export type TaskRow = z.infer<typeof TaskRowSchema>;
/** Live checker card of one Tester check: start, progress, error or done (done removes the card). */
export const CheckerActivitySchema = z.object({
  id: z.string(), phase: z.enum(["start", "progress", "error", "done"]),
  title: z.string().default(""), attempt: z.number().default(0),
  model: z.string().default(""), provider_name: z.string().default("Người kiểm tra"), text: z.string().default(""),
});
export type CheckerActivity = z.infer<typeof CheckerActivitySchema>;
export const StreamDataSchema = z.object({
  tasks: z.array(TaskRowSchema).optional(),
  checker_activity: CheckerActivitySchema.optional(),
  text: z.string().optional(), message: z.string().optional(), provider_name: z.string().optional(), provider_kind: z.string().optional(), model: z.string().optional(),
  effort: z.string().optional(), role: z.string().nullish(), elapsed_s: z.number().optional(),
  idle_s: z.number().optional(), last_activity: z.string().optional(), scope: z.string().optional(),
  action: z.string().optional(), status: z.string().optional(), files: z.array(FileChangeSchema).optional(),
  /** `permission_denied`: the one level up that would let the blocked step run. */
  need: z.enum(["write", "full"]).optional(),
  /** `permission_request`: the run waits for the user to allow or refuse `command`. */
  id: z.string().optional(), command: z.string().optional(),
});
export type StreamData = z.infer<typeof StreamDataSchema>;
