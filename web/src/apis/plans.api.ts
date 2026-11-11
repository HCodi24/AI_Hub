import { z } from "zod";
import { http, parseResponse } from "@/lib/http";
import { postSse } from "@/lib/sse";
import { PlanSchema, PlanInputSchema, VerifySchema, type Plan } from "@/schema/plans.schema";
import { StreamDataSchema } from "@/schema/chat.schema";
const pendingSaves = new Map<string, Promise<unknown>>();
export const plansApi = {
  async list(signal?: AbortSignal) { return parseResponse(z.array(PlanSchema), await http.get("/plans", signal)); },
  async get(id: string, signal?: AbortSignal) { return parseResponse(PlanSchema, await http.get(`/plans/${encodeURIComponent(id)}`, signal)); },
  async save(plan: Plan) {
    const snapshot = PlanSchema.parse(plan);
    const previous = pendingSaves.get(plan.id) || Promise.resolve();
    const saving = previous.catch(() => undefined).then(async () => parseResponse(PlanSchema,
      await (await http.request(`/plans/${encodeURIComponent(plan.id)}`, { method: "PUT", body: JSON.stringify(snapshot) })).json()));
    pendingSaves.set(plan.id, saving);
    try { return await saving; }
    finally { if (pendingSaves.get(plan.id) === saving) pendingSaves.delete(plan.id); }
  },
  generate(body: z.input<typeof PlanInputSchema>, signal: AbortSignal, event: (name: string, data: unknown) => void) { return postSse("/plans", PlanInputSchema.parse(body), signal, event, "plan"); },
  async verify(folder: string, signal: AbortSignal, log: (text: string) => void = () => {}) {
    let report: z.infer<typeof VerifySchema> | undefined;
    await postSse("/verify", { folder }, signal, (event, data) => {
      if (event === "verify") report = parseResponse(VerifySchema, data);
      if (event === "log") log(parseResponse(StreamDataSchema, data).text || "");
    }, "verify");
    if (!report) throw new Error("Thiếu kết quả xác minh");
    return report;
  },
  async diff(folder: string) { return parseResponse(z.object({ diff: z.string() }), await (await http.request("/plan-diff", { method: "POST", body: JSON.stringify({ folder }) })).json()); },
};
