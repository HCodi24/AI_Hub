import { http, parseResponse } from "@/lib/http";
import { DemoFilesSchema, DemoRemoveSchema } from "@/schema/demo.schema";

const url = (folder: string) => `/demo?folder=${encodeURIComponent(folder)}`;

export const demoApi = {
  async list(folder: string, signal?: AbortSignal) {
    return parseResponse(DemoFilesSchema, await http.get(url(folder), signal));
  },
  async remove(folder: string) {
    const response = await http.request(url(folder), { method: "DELETE" });
    return parseResponse(DemoRemoveSchema, await response.json());
  },
};
