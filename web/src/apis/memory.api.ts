import { http, parseResponse } from "@/lib/http";
import { MemoryOverviewSchema, MemoryTextSchema, MemoryMutationSchema, MemoryProjectsSchema } from "@/schema/memory.schema";
const url = (folder: string, scope = "", action = "") => `/memory${scope ? `/${encodeURIComponent(scope)}` : ""}${action}?folder=${encodeURIComponent(folder)}`;
export const memoryApi = {
  async overview(folder: string, signal?: AbortSignal) { return parseResponse(MemoryOverviewSchema, await http.get(url(folder), signal)); },
  async get(folder: string, scope: string, signal?: AbortSignal) { return parseResponse(MemoryTextSchema, await http.get(url(folder, scope), signal)); },
  async put(folder: string, scope: string, text: string) { return parseResponse(MemoryTextSchema, await (await http.request(url(folder, scope), { method: "PUT", body: JSON.stringify(MemoryTextSchema.parse({ text })) })).json()); },
  async archive(folder: string, scope: string) { parseResponse(MemoryMutationSchema, await (await http.request(url(folder, scope, "/archive"), { method: "POST" })).json()); },
  async restore(folder: string, scope: string) { parseResponse(MemoryMutationSchema, await (await http.request(url(folder, scope, "/restore"), { method: "POST" })).json()); },
  async projects(signal?: AbortSignal) { return parseResponse(MemoryProjectsSchema, await http.get("/memory/projects", signal)).projects; },
  /** Text to attach to the next message: a session of `folder`, or another project's summary. */
  async recall(target: { folder: string; session: string } | { project: string }) {
    return parseResponse(MemoryTextSchema, await http.get(`/memory/recall?${new URLSearchParams(target)}`)).text;
  },
  async delete(folder: string, scope: string) { parseResponse(MemoryMutationSchema, await (await http.request(url(folder, scope), { method: "DELETE" })).json()); },
};
