import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { ConfigSchema } from "@/schema/config.schema";
export const configApi = {
  async get(signal?: AbortSignal) { return parseResponse(ConfigSchema, await http.get("/config", signal)); },
  /** The theme lives in the server's config file, so it does not depend on the page's address. */
  async saveTheme(theme: string) { await http.request("/config", { method: "PUT", body: JSON.stringify({ theme }) }); },
  /** Opens the native folder dialog on this machine; resolves to null when the user cancels. */
  async pickFolder() {
    const data = await (await http.request("/pick-folder", { method: "POST" })).json();
    return parseResponse(z.object({ path: z.string().nullable() }), data).path;
  },
};
