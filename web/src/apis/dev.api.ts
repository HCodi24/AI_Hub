import { http, parseResponse } from "@/lib/http";
import { streamEvents } from "@/lib/sse";
import { DevDiffSchema, DevGitResultSchema, DevRepoSchema, DevSettingsSchema, DevStatusSchema } from "@/schema/dev.schema";
const post = async (path: string, body?: unknown) =>
  (await http.request(path, { method: "POST", ...(body === undefined ? {} : { body: JSON.stringify(body) }) })).json() as Promise<unknown>;
export const devApi = {
  async status(signal?: AbortSignal) { return parseResponse(DevStatusSchema, await http.get("/dev/status", signal)); },
  async repo(signal?: AbortSignal) { return parseResponse(DevRepoSchema, await http.get("/dev/repo", signal)); },
  async diff() { return parseResponse(DevDiffSchema, await http.get("/dev/git/diff")); },
  async checkpoint() { return parseResponse(DevGitResultSchema, await post("/dev/git/checkpoint")); },
  async reset() { return parseResponse(DevGitResultSchema, await post("/dev/git/reset")); },
  async commit(message: string) { return parseResponse(DevGitResultSchema, await post("/dev/git/commit", { message })); },
  async settings(signal?: AbortSignal) { return parseResponse(DevSettingsSchema, await http.get("/dev/settings", signal)); },
  async saveSettings(auto_rebuild: boolean) {
    return parseResponse(DevSettingsSchema, await (await http.request("/dev/settings", { method: "PUT", body: JSON.stringify({ auto_rebuild }) })).json());
  },
  /** Streams `log` / `summary` / `verify` / `rebuild` events until the command ends. */
  run(kind: "test" | "release" | "rebuild" | "web-build", signal: AbortSignal, onEvent: (event: string, data: unknown) => void) {
    return streamEvents(`/dev/${kind}`, undefined, signal, onEvent);
  },
};
