import { create } from "zustand";
import type { DebateLive } from "@/components/features/debate/debate-reducer";
type CardState = { debates: Record<string, DebateLive>; running: string | null; debate: (id: string, debate: DebateLive) => void };
export const useCardStore = create<CardState>((set) => ({ debates: {}, running: null,
  debate: (id, debate) => set(state => ({ debates: { ...state.debates, [id]: debate } })),
}));
