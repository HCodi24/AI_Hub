import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { postSse } from "@/lib/sse";
import { DebateSchema, DebateInputSchema } from "@/schema/debates.schema";
export const debatesApi = {
  async list(signal?: AbortSignal) { return parseResponse(z.array(DebateSchema), await http.get("/debates", signal)); },
  async get(id: string, signal?: AbortSignal) { return parseResponse(DebateSchema, await http.get(`/debates/${encodeURIComponent(id)}`, signal)); },
  start(body: z.input<typeof DebateInputSchema>, signal: AbortSignal, event: (name: string, data: unknown) => void) { return postSse("/debates", DebateInputSchema.parse(body), signal, event); },
  async link(id: string, plan_id: string) { return parseResponse(DebateSchema, await (await http.request(`/debates/${encodeURIComponent(id)}/plan`, { method: "POST", body: JSON.stringify(z.object({ plan_id: z.string().uuid() }).parse({ plan_id })) })).json()); },
};
