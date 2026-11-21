import { http, parseResponse } from "@/lib/http";
import { streamEvents } from "@/lib/sse";
import { DevPreviewServerSchema, DevRepoSchema, DevStatusSchema } from "@/schema/dev.schema";
const post = async (path: string, body?: unknown) =>
  (await http.request(path, { method: "POST", ...(body === undefined ? {} : { body: JSON.stringify(body) }) })).json() as Promise<unknown>;
export const devApi = {
  async status(signal?: AbortSignal) { return parseResponse(DevStatusSchema, await http.get("/dev/status", signal)); },
  async repo(signal?: AbortSignal) { return parseResponse(DevRepoSchema, await http.get("/dev/repo", signal)); },
  /** The live preview (Next dev) server: whether it listens, and whether this app started it and can stop it. */
  async previewServer(signal?: AbortSignal) { return parseResponse(DevPreviewServerSchema, await http.get("/dev/preview-server", signal)); },
  async previewServerAction(action: "start" | "stop") { return parseResponse(DevPreviewServerSchema, await post("/dev/preview-server", { action })); },
  /** Streams `log` / `summary` / `verify` / `rebuild` events until the command ends. */
  run(kind: "test" | "release" | "rebuild" | "web-build", signal: AbortSignal, onEvent: (event: string, data: unknown) => void) {
    return streamEvents(`/dev/${kind}`, undefined, signal, onEvent);
  },
};
