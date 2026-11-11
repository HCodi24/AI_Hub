import { http, parseResponse } from "@/lib/http";
import { ConfigSchema } from "@/schema/config.schema";
export const configApi = {
  async get(signal?: AbortSignal) { return parseResponse(ConfigSchema, await http.get("/config", signal)); },
};
