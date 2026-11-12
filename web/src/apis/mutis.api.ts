import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { ApplyResultSchema, DiagnosesSchema, MutisSchema, MutisStatsSchema, ResolvedRolesSchema, type Mutis } from "@/schema/mutis.schema";
import { streamEvents } from "@/lib/sse";
export const mutisApi = {
  async get(signal?: AbortSignal) { return parseResponse(MutisSchema, await http.get("/mutis", signal)); },
  async put(body: Mutis) { return parseResponse(MutisSchema, await (await http.request("/mutis", { method: "PUT", body: JSON.stringify(MutisSchema.parse(body)) })).json()); },
  async stats(days = 30, signal?: AbortSignal) { return parseResponse(MutisStatsSchema, await http.get(`/mutis/stats?days=${days}`, signal)); },
  async diagnoses(signal?: AbortSignal) { return parseResponse(DiagnosesSchema, await http.get("/mutis/diagnoses", signal)); },
  /** AI diagnosis of the MutiS event log: streams `log`, `error` and a final `diagnosis` event. */
  diagnose(signal: AbortSignal, onEvent: (event: string, data: unknown) => void) { return streamEvents("/mutis/diagnose", undefined, signal, onEvent); },
  async apply(change: { path: string; value: unknown }) {
    return parseResponse(ApplyResultSchema, await (await http.request("/mutis/apply", { method: "POST", body: JSON.stringify(change) })).json());
  },
  /** `effort` picks the level: max/xhigh = 3 AIs, otherwise Codex + DeepSeek only. */
  async resolve(effort?: string) { return parseResponse(ResolvedRolesSchema, await http.get(`/mutis/roles/resolve${effort ? `?effort=${encodeURIComponent(effort)}` : ""}`)); },
  async event(body: { plan_id: string; task_id: string; task_kind: string; provider_id: string; kind: "verify" | "review" | "fix_task"; ok: boolean; detail: string }) {
    return parseResponse(z.object({ ok: z.boolean() }), await (await http.request("/mutis/events", { method: "POST", body: JSON.stringify(body) })).json());
  },
};
