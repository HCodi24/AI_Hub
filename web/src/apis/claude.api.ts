import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
const StatusSchema = z.object({ connected: z.boolean(), pending: z.boolean(), message: z.string(), project_skills: z.boolean(), project_instructions: z.boolean() });
export type ClaudeStatus = z.infer<typeof StatusSchema>;
const path = (id: string) => `/providers/${encodeURIComponent(id)}/claude`;
export const claudeApi = {
  async status(id: string, signal?: AbortSignal) { return parseResponse(StatusSchema, await http.get(path(id), signal)); },
  async action(id: string, action: "cli" | "logout") {
    return parseResponse(StatusSchema, await (await http.request(path(id), { method: "POST", body: JSON.stringify({ action }) })).json());
  },
  async settings(id: string, value: Pick<ClaudeStatus, "project_skills" | "project_instructions">) {
    return parseResponse(StatusSchema, await (await http.request(path(id), { method: "PUT", body: JSON.stringify(value) })).json());
  },
};
export function safeClaudeLoginUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password
      && ["claude.ai", "claude.com", "anthropic.com"].some(domain => url.hostname === domain || url.hostname.endsWith(`.${domain}`)) ? url.href : null;
  } catch { return null; }
}
