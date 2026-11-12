import { z } from "zod";
import { ProviderSummarySchema } from "@/schema/config.schema";
import { EffortSchema } from "@/schema/chat.schema";
export const RoleSchema = z.object({
    provider_id: z.string().default(""),
    model: z.string().default(""),
    effort: EffortSchema.default("default")
});
export const MutisSchema = z.object({
    enabled: z.boolean(),
    worker_rules: z.string().default(""),
    auto_plan: z.boolean().default(true),
    verify_gate: z.boolean().default(false),
    auto_review: z.boolean().default(false),
    auto_fix: z.boolean().default(false),
    fix_rounds_max: z.number().default(2),
    retry_api: z.boolean().default(false),
    retry_attempts: z.number().default(3),
    retry_base_ms: z.number().default(2000),
    codex_watchdog: z.boolean().default(false),
    watchdog_minutes: z.number().default(5),
    codex_auto_resume: z.boolean().default(false),
    resume_max: z.number().default(2),
    deepseek_agent: z.boolean().default(false),
    deepseek_max_file_lines: z.number().default(400),
    deepseek_max_steps: z.number().default(15),
    debate_rounds: z.number().default(2),
    debate_judge_model: z.string().default(""),
    verify_command: z.string().default(""),
    verify_ignore: z.array(z.string()).default([]),
    reviewer_provider_id: z.string().default(""),
    reviewer_model: z.string().default(""),
    event_log: z.boolean().default(true),
    roles: z.object({
        lead: RoleSchema,
        big: RoleSchema,
        small: RoleSchema
    }),
    high_roles: z.object({ lead: RoleSchema, big: RoleSchema, small: RoleSchema }).default({ lead: { provider_id: "", model: "", effort: "default" }, big: { provider_id: "", model: "", effort: "default" }, small: { provider_id: "", model: "", effort: "default" } }),
    routing: z.array(z.object({
        kind: z.string(),
        role: z.string().default(""),
        assignee_kind: z.string().default(""),
        note: z.string().default("")
    })).default([])
}).passthrough();
const ResolvedRoleSchema = z.object({
    used_provider: ProviderSummarySchema.nullish()
});
export const ResolvedRolesSchema = z.object({
    lead: ResolvedRoleSchema,
    big: ResolvedRoleSchema,
    small: ResolvedRoleSchema
});
export const DiagnosisSchema = z.object({
    ts: z.number().default(0), health: z.string().default(""), summary: z.string().default(""),
    findings: z.array(z.object({
        severity: z.string().default("low"), problem: z.string().default(""), evidence: z.string().default(""),
        cause: z.string().default(""), suggestion: z.string().default(""),
        setting_change: z.object({ path: z.string(), value: z.unknown() }).nullish()
    }).passthrough()).default([])
}).passthrough();
export const DiagnosesSchema = z.array(DiagnosisSchema);
export const ApplyResultSchema = z.object({ old: z.unknown(), new: z.unknown() });
export type Diagnosis = z.infer<typeof DiagnosisSchema>;
export type Mutis = z.infer<typeof MutisSchema>;
const StatGroupSchema = z.object({
    tasks_run: z.number().default(0), success_rate: z.number().default(0), avg_duration_ms: z.number().default(0),
    total_tokens: z.number().default(0), retries: z.number().default(0), watchdog_kills: z.number().default(0),
    resumes: z.number().default(0), verify_fail_rate: z.number().default(0), review_fail_rate: z.number().default(0),
    fix_rounds: z.number().default(0),
    top_errors: z.array(z.object({ message: z.string(), count: z.number() })).default([])
}).passthrough();
export const MutisStatsSchema = z.object({
    days: z.number(),
    providers: z.record(StatGroupSchema).default({}),
    task_kinds: z.record(StatGroupSchema).default({})
}).passthrough();
export type MutisStats = z.infer<typeof MutisStatsSchema>;
