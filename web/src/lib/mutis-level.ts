import type { Mutis } from "@/schema/mutis.schema";

/** MutiS has one level, High: every effort uses `high_roles` (the old `roles` only while High has no lead of its own). */
export function rolesFor(mutis: Mutis | undefined, _effort?: string): Mutis["roles"] | undefined {
  if (!mutis) return undefined;
  return mutis.high_roles?.lead.provider_id ? mutis.high_roles : mutis.roles;
}
