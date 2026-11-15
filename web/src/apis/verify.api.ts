import { z } from "zod";
import { http, parseResponse } from "@/lib/http";

const ProjectCommandsSchema = z.object({
  folder: z.string(), saved: z.string().nullish(), global: z.string().default(""),
  detected: z.array(z.object({ name: z.string(), command: z.string() })).default([]),
});
export type ProjectCommands = z.infer<typeof ProjectCommandsSchema>;
/** Verify command of one project folder: what is saved, the global fallback and what auto-detection would run. */
export const verifyApi = {
  async get(folder: string, signal?: AbortSignal) { return parseResponse(ProjectCommandsSchema, await http.get(`/verify/commands?folder=${encodeURIComponent(folder)}`, signal)); },
  /** An empty command removes the project's own command. */
  async put(folder: string, command: string) {
    return parseResponse(ProjectCommandsSchema, await (await http.request("/verify/commands", { method: "PUT", body: JSON.stringify({ folder, command }) })).json());
  },
};
