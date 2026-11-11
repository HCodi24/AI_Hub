import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { MutisSchema, ResolvedRolesSchema, type Mutis } from "@/schema/mutis.schema";
export const mutisApi = {
  async get(signal?: AbortSignal) { return parseResponse(MutisSchema, await http.get("/mutis", signal)); },
  async put(body: Mutis) { return parseResponse(MutisSchema, await (await http.request("/mutis", { method: "PUT", body: JSON.stringify(MutisSchema.parse(body)) })).json()); },
  async resolve() { return parseResponse(ResolvedRolesSchema, await http.get("/mutis/roles/resolve")); },
  async event(body: { plan_id: string; task_id: string; task_kind: string; provider_id: string; kind: "verify" | "review" | "fix_task"; ok: boolean; detail: string }) {
    return parseResponse(z.object({ ok: z.boolean() }), await (await http.request("/mutis/events", { method: "POST", body: JSON.stringify(body) })).json());
  },
};
