import { z } from "zod";
export const ScopeSchema = z.object({
    scope: z.string(),
    title: z.string(),
    status: z.string(),
    updated: z.string().default(""),
    next: z.string().default("")
});
export const MemoryOverviewSchema = z.object({
    project: z.string(),
    index: z.string(),
    scopes: z.array(ScopeSchema),
    archived: z.array(ScopeSchema)
});
export const MemoryTextSchema = z.object({
    text: z.string()
});
export const MemoryEventSchema = z.object({
    scope: z.string(),
    status: z.string(),
    action: z.enum(["created", "updated", "archived"])
});
export type MemoryEvent = z.infer<typeof MemoryEventSchema>;

export const MemoryMutationSchema = z.object({ ok: z.boolean() });
