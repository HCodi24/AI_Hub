import { z } from "zod";
import { PermissionSchema, EffortSchema, ChatRequestSchema } from "@/schema/chat.schema";
export const VerifySchema = z.object({
    ok: z.boolean(),
    summary_lines: z.array(z.string()).default([])
}).passthrough();
export const ReviewSchema = z.object({
    verdict: z.enum(["PASS", "FAIL", "SKIPPED"]),
    issues: z.array(z.object({
        severity: z.string(),
        problem: z.string(),
        file: z.string().default(""),
        line: z.number().default(0),
        fix: z.string().default("")
    })).default([]),
    notes: z.string().default("")
});
export const TaskSchema = z.object({
    id: z.number(),
    title: z.string(),
    detail: z.string(),
    assignee: z.string(),
    permission: PermissionSchema.nullish(),
    model: z.string().nullish(),
    effort: EffortSchema.default("default"),
    reassigned_from: z.string().nullish(),
    depends_on: z.array(z.number()).default([]),
    kind: z.enum(["plan", "small_edit", "code", "test", "docs", "review"]),
    status: z.string().default("chờ"),
    summary: z.string().default(""),
    diff: z.string().default(""),
    log: z.array(z.string()).default([]),
    tokens: z.number().default(0),
    chat_id: z.string().nullish(),
    verify: VerifySchema.nullish(),
    review: ReviewSchema.nullish(),
    fix_rounds_used: z.number().default(0),
    fix_root_title: z.string().nullish(),
    fix_root_detail: z.string().nullish()
});
export const PlanSchema = z.object({
    id: z.string(),
    chat_id: z.string().nullish(),
    title: z.string(),
    folder: z.string(),
    dev_chat: z.boolean().default(false),
    request: z.string(),
    level: z.enum(["high", "max"]).default("max"),
    approved: z.boolean().default(false),
    planning: z.boolean().default(false),
    error: z.string().nullish(),
    tasks: z.array(TaskSchema).default([]),
    updated_at: z.number().default(0),
    auto_continue: z.boolean().default(false)
});
export const PlanInputSchema = z.object({
    request: z.string().min(1),
    folder: z.string().min(1),
    chat_id: z.string().uuid(),
    append_request: z.boolean().default(true),
    after_debate_id: z.string().nullish(),
    dev_chat: z.boolean().default(false),
    provider_id: z.string().default(""),
    model: z.string().default(""),
    effort: EffortSchema.default("max")
});
export type Plan = z.infer<typeof PlanSchema>;
export type PlanTask = z.infer<typeof TaskSchema>;
export type Review = z.infer<typeof ReviewSchema>;

export const PlanChatRequestSchema = ChatRequestSchema.extend({
    plan_id: z.string().uuid(),
    task_id: z.string(),
    task_kind: z.string()
});
