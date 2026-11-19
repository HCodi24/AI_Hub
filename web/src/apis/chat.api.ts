import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { postSse } from "@/lib/sse";
import { ChatRecordSchema, ChatSummarySchema, ChatRequestSchema, StreamDataSchema, type ChatRequest, type StreamData } from "@/schema/chat.schema";

/** Counts must be exact nonnegative integers: a rounded, fractional or negative value is corrupt. */
const tokenCount = z.number().refine(value => Number.isSafeInteger(value) && value >= 0, { message: "invalid token count" });
/** Largest timestamp a JS Date can represent; anything beyond it cannot be rendered. */
export const MAX_TOKEN_TIMESTAMP = 8640000000000000;
const tokenTimestamp = z.number()
  .refine(value => Number.isSafeInteger(value) && value >= 0 && value <= MAX_TOKEN_TIMESTAMP, { message: "invalid timestamp" });
const TokenHistoryRowSchema = z.object({
  model: z.string(),
  input: tokenCount,
  output: tokenCount,
  total: tokenCount,
}).refine(row => row.total === row.input + row.output, { message: "total must equal input + output" });
const TokenHistorySampleSchema = z.object({
  at: tokenTimestamp,
  models: z.array(TokenHistoryRowSchema).min(1),
}).refine(sample => new Set(sample.models.map(row => row.model)).size === sample.models.length, { message: "duplicate model identity" })
  .transform(sample => ({ at: sample.at, models: [...sample.models].sort((left, right) => left.model.localeCompare(right.model)) }));
/** Backend history: strictly ordered snapshots, one row per model, no invented points. */
export const TokenHistorySchema = z.array(TokenHistorySampleSchema)
  .refine(samples => samples.every((sample, index) => index === 0 || sample.at > samples[index - 1].at), { message: "timestamps must be strictly increasing" });
export type TokenHistorySample = z.output<typeof TokenHistorySchema>[number];

export const chatApi = {
  async list(signal?: AbortSignal) { return parseResponse(z.array(ChatSummarySchema), await http.get("/chats", signal)); },
  async get(id: string, signal?: AbortSignal) { return parseResponse(ChatRecordSchema, await http.get(`/chats/${encodeURIComponent(id)}`, signal)); },
  /** Recorded token snapshots of one chat (latest 500). A malformed body throws instead of becoming []. */
  async tokenHistory(id: string, signal?: AbortSignal) {
    return parseResponse(TokenHistorySchema, await http.get(`/chats/${encodeURIComponent(id)}/token-history`, signal));
  },
  async stop(id: string) { await http.request(`/chat/${encodeURIComponent(id)}/stop`, { method: "POST" }); },
  async delete(id: string) { await http.request(`/chats/${encodeURIComponent(id)}`, { method: "DELETE" }); },
  /** Compacts every Codex thread of the chat; `failed` holds the reason of each thread that could not be compacted. */
  async compact(id: string) {
    const response = await http.request(`/chats/${encodeURIComponent(id)}/compact`, { method: "POST" });
    return parseResponse(z.object({ compacted: z.number(), failed: z.array(z.string()) }), await response.json());
  },
  async send(body: ChatRequest, signal: AbortSignal, onEvent: (event: string, data: StreamData) => void) {
    await postSse("/chat", ChatRequestSchema.parse(body), signal, (event, data) => onEvent(event, parseResponse(StreamDataSchema, data)));
  },
};
