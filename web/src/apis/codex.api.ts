import { z } from "zod";
import { http, parseResponse } from "@/lib/http";

const StatusSchema = z.object({
  connected: z.boolean(), pending: z.boolean(), url: z.string().nullish(), message: z.string(),
  project_skills: z.boolean(), project_instructions: z.boolean(),
  /** Windows sandbox of the MutiS Codex home: without it Codex cannot write files. */
  sandbox: z.string().default("unknown"),
});
export type CodexStatus = z.infer<typeof StatusSchema>;
const path = (id: string) => `/providers/${encodeURIComponent(id)}/codex`;
export const codexApi = {
  async status(id: string, signal?: AbortSignal) { return parseResponse(StatusSchema, await http.get(path(id), signal)); },
  async action(id: string, action: "cli" | "web" | "cancel" | "logout" | "sandbox") {
    return parseResponse(StatusSchema, await (await http.request(path(id), { method: "POST", body: JSON.stringify({ action }) })).json());
  },
  async settings(id: string, settings: Pick<CodexStatus, "project_skills" | "project_instructions">) {
    return parseResponse(StatusSchema, await (await http.request(path(id), { method: "PUT", body: JSON.stringify(settings) })).json());
  },
};

export function safeCodexLoginUrl(value?: string | null): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password
      && ["auth.openai.com", "auth0.openai.com", "chatgpt.com"].includes(url.hostname) ? url.href : null;
  } catch { return null; }
}
