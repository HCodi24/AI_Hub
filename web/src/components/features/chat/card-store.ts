import { create } from "zustand";
import type { Plan } from "@/schema/plans.schema";
import type { DebateLive } from "@/components/features/debate/debate-reducer";
type CardState = { plans: Record<string, Plan>; debates: Record<string, DebateLive>; running: string | null; logs: Record<string, string[]>; confirm: string | null;
  plan: (plan: Plan) => void; debate: (id: string, debate: DebateLive) => void; log: (id: string, text: string) => void };
export const useCardStore = create<CardState>((set) => ({ plans: {}, debates: {}, running: null, logs: {}, confirm: null,
  plan: plan => set(state => ({ plans: { ...state.plans, [plan.id]: { ...plan, tasks: [...plan.tasks] } } })),
  debate: (id, debate) => set(state => ({ debates: { ...state.debates, [id]: debate } })),
  log: (id, text) => set(state => ({ logs: { ...state.logs, [id]: [...(state.logs[id] || []), text] } })),
}));
