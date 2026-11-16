import { http, parseResponse } from "@/lib/http";
import { AddonChangeResultSchema, AddonsSchema, CodexPluginsSchema, LinkFoundSchema, type AddonChange, type LinkTarget } from "@/schema/addons.schema";
export const addonsApi = {
  async overview(signal?: AbortSignal) { return parseResponse(AddonsSchema, await http.get("/addons", signal)); },
  /** Copies one add-on from the user's own Claude Code (or removes it). Plugin installs can take a minute. */
  async change(body: AddonChange) {
    return parseResponse(AddonChangeResultSchema, await (await http.request("/addons", { method: "POST", body: JSON.stringify(body) })).json());
  },
  async codexPlugins(signal?: AbortSignal) { return parseResponse(CodexPluginsSchema, await http.get("/addons/codex", signal)); },
  /** Downloads the GitHub repository and reports which CLIs can install it. Nothing is installed. */
  async inspectLink(link: string) {
    return parseResponse(LinkFoundSchema, await (await http.request("/addons/link/inspect", { method: "POST", body: JSON.stringify({ link }) })).json());
  },
  async installLink(body: { link: string; targets: LinkTarget[]; plugins: string[] }) {
    return parseResponse(AddonChangeResultSchema, await (await http.request("/addons/link/install", { method: "POST", body: JSON.stringify(body) })).json());
  },
};
