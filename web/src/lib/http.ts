import { z } from "zod";
import { chatText } from "@/text/chat";
export class ApiError extends Error {
  constructor(message: string, public readonly status: number) { super(message); this.name = "ApiError"; }
}
export const http = {
  async request(path: string, init?: RequestInit): Promise<Response> {
    let response: Response;
    try {
      response = await fetch(`/api${path}`, { ...init, headers: { "Content-Type": "application/json", ...init?.headers } });
    } catch (error: unknown) {
      if (error instanceof DOMException && error.name === "AbortError") throw error;
      throw new ApiError(chatText.requestError, 0);
    }
    if (!response.ok) {
      const data = z.object({ error: z.string() }).safeParse(await response.json().catch(() => null));
      throw new ApiError(data.success ? data.data.error : chatText.requestError, response.status);
    }
    return response;
  },
  async get(path: string, signal?: AbortSignal): Promise<unknown> {
    return (await http.request(path, { signal })).json();
  },
};
export function parseResponse<T extends z.ZodTypeAny>(schema: T, data: unknown): z.output<T> {
  const result = schema.safeParse(data);
  if (!result.success) throw new ApiError(chatText.invalidData, 0);
  return result.data;
}
