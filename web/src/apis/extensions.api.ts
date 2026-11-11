import { http, parseResponse } from "@/lib/http";
import { streamEvents } from "@/lib/sse";
import { ExtensionsSchema } from "@/schema/extensions.schema";
export const extensionsApi = {
  async list(folder: string, signal?: AbortSignal) {
    return parseResponse(ExtensionsSchema, await http.get(`/extensions${folder ? `?folder=${encodeURIComponent(folder)}` : ""}`, signal));
  },
  run(extension: string, action: string, folder: string, signal: AbortSignal, onEvent: (event: string, data: unknown) => void) {
    return streamEvents(`/extensions/${encodeURIComponent(extension)}/actions/${encodeURIComponent(action)}`, { folder }, signal, onEvent);
  },
  async stop(extension: string) { await http.request(`/extensions/${encodeURIComponent(extension)}/stop`, { method: "POST" }); },
};
