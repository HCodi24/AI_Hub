import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { postSse } from "@/lib/sse";
import { ChatRecordSchema, ChatSummarySchema, ChatRequestSchema, StreamDataSchema, type ChatRequest, type StreamData } from "@/schema/chat.schema";
export const chatApi = {
  async list(signal?: AbortSignal) { return parseResponse(z.array(ChatSummarySchema), await http.get("/chats", signal)); },
  async get(id: string, signal?: AbortSignal) { return parseResponse(ChatRecordSchema, await http.get(`/chats/${encodeURIComponent(id)}`, signal)); },
  async stop(id: string) { await http.request(`/chat/${encodeURIComponent(id)}/stop`, { method: "POST" }); },
  async delete(id: string) { await http.request(`/chats/${encodeURIComponent(id)}`, { method: "DELETE" }); },
  async send(body: ChatRequest, signal: AbortSignal, onEvent: (event: string, data: StreamData) => void) {
    await postSse("/chat", ChatRequestSchema.parse(body), signal, (event, data) => onEvent(event, parseResponse(StreamDataSchema, data)));
  },
};
