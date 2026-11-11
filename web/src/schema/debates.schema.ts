import { z } from "zod";
export const TurnSchema = z.object({
    text: z.string().default(""),
    agree: z.boolean().nullish(),
    disagreements: z.array(z.string()).default([]),
    tokens: z.number().default(0),
    elapsed_s: z.number().default(0),
    error: z.string().nullish()
});
export const RoundSchema = z.object({
    index: z.number(),
    codex: TurnSchema.default({}),
    claude: TurnSchema.default({})
});
export const VerdictSchema = z.object({
    decision: z.string().default(""),
    chosen: z.string().default("merged"),
    agreed_points: z.array(z.string()).default([]),
    disagreements: z.array(z.string()).default([]),
    risks: z.array(z.string()).default([]),
    plan_steps: z.array(z.string()).default([]),
    tokens: z.number().default(0)
});
export const DebateSchema = z.object({
    id: z.string(),
    title: z.string().default(""),
    request: z.string().default(""),
    folder: z.string().default(""),
    chat_id: z.string().nullish(),
    plan_id: z.string().nullish(),
    created_at: z.number().default(0),
    status: z.string().default("running"),
    elapsed_s: z.number().default(0),
    codex_provider: z.string().default(""),
    claude_provider: z.string().default(""),
    codex_model: z.string().default(""),
    claude_model: z.string().default(""),
    judge_model: z.string().default(""),
    max_rounds: z.number().default(2),
    rounds: z.array(RoundSchema).default([]),
    verdict: VerdictSchema.nullish(),
    error: z.string().nullish(),
    plan_request: z.string().default("")
});
export const DebateInputSchema = z.object({
    request: z.string().min(1),
    folder: z.string().min(1),
    chat_id: z.string().uuid()
});
export const DebateEventSchema = z.object({
    side: z.enum(["codex", "claude", "judge"]).optional(),
    round: z.number().optional(),
    text: z.string().nullish(),
    model: z.string().optional(),
    turn: TurnSchema.optional()
});
export type Debate = z.infer<typeof DebateSchema>;
export type Turn = z.infer<typeof TurnSchema>;
