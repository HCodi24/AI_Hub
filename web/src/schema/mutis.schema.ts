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
    roles: z.object({
        lead: RoleSchema,
        big: RoleSchema,
        small: RoleSchema
    }),
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
export type Mutis = z.infer<typeof MutisSchema>;
