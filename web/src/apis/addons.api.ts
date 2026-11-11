import { http, parseResponse } from "@/lib/http";
import { AddonChangeResultSchema, AddonsSchema, type AddonChange } from "@/schema/addons.schema";
export const addonsApi = {
  async overview(signal?: AbortSignal) { return parseResponse(AddonsSchema, await http.get("/addons", signal)); },
  /** Copies one add-on from the user's own Claude Code (or removes it). Plugin installs can take a minute. */
  async change(body: AddonChange) {
    return parseResponse(AddonChangeResultSchema, await (await http.request("/addons", { method: "POST", body: JSON.stringify(body) })).json());
  },
};
