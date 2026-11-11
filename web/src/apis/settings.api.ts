import { http, parseResponse } from "@/lib/http";
import {
  CliCandidatesSchema, CliStatusSchema, ConnectionTestSchema, FullConfigSchema, LoginResultSchema, LoginStartSchema,
  ModelsSchema, type MemoryConfig, type ProviderInput,
} from "@/schema/settings.schema";
const send = async (path: string, method: string, body?: unknown) =>
  (await http.request(path, { method, ...(body === undefined ? {} : { body: JSON.stringify(body) }) })).json() as Promise<unknown>;
const id = (value: string) => encodeURIComponent(value);
export const settingsApi = {
  async config(signal?: AbortSignal) { return parseResponse(FullConfigSchema, await http.get("/config", signal)); },
  /** Partial update: the server keeps every field that is not sent (default_folder is always required). */
  async saveConfig(body: { default_folder: string; chat_idle_timeout_minutes?: number; memory?: MemoryConfig;
    planner_provider_id?: string; planner_model?: string }) {
    return parseResponse(FullConfigSchema, await send("/config", "PUT", body));
  },
  async createProvider(body: ProviderInput) { return parseResponse(FullConfigSchema, await send("/providers", "POST", body)); },
  async updateProvider(provider: string, body: ProviderInput) {
    return parseResponse(FullConfigSchema, await send(`/providers/${id(provider)}`, "PUT", body));
  },
  async deleteProvider(provider: string) { return parseResponse(FullConfigSchema, await send(`/providers/${id(provider)}`, "DELETE")); },
  async testProvider(provider: string) { return parseResponse(ConnectionTestSchema, await send(`/providers/${id(provider)}/test`, "POST")); },
  async models(provider: string) { return parseResponse(ModelsSchema, await http.get(`/providers/${id(provider)}/models`)); },
  async cliStatus(signal?: AbortSignal) { return parseResponse(CliStatusSchema, await http.get("/cli-status", signal)); },
  async discoverCli(name: "codex" | "claude") { return parseResponse(CliCandidatesSchema, await http.get(`/cli-discover/${name}`)); },
  async loginStart(provider: string) { return parseResponse(LoginStartSchema, await send(`/providers/${id(provider)}/login/start`, "POST")); },
  async loginCode(provider: string, session_id: string, code: string) {
    return parseResponse(LoginResultSchema, await send(`/providers/${id(provider)}/login/code`, "POST", { session_id, code }));
  },
  async loginCancel(provider: string) { await send(`/providers/${id(provider)}/login/cancel`, "POST"); },
};
