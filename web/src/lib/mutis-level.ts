import type { Mutis } from "@/schema/mutis.schema";

export function levelFor(effort: string): "high" | "max" { return effort === "max" || effort === "xhigh" ? "max" : "high"; }
export function rolesFor(mutis: Mutis | undefined, effort: string): Mutis["roles"] | undefined {
  if (!mutis) return undefined;
  return levelFor(effort) === "max" || !mutis.high_roles?.lead.provider_id ? mutis.roles : mutis.high_roles;
}
