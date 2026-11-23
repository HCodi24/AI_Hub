import { create } from "zustand";
import { persist } from "zustand/middleware";

export const CHAT_PET_SIZES = ["small", "medium", "large"] as const;
type ChatPetSize = (typeof CHAT_PET_SIZES)[number];

export const useChatPetSize = create<{ size: ChatPetSize; setSize: (size: ChatPetSize) => void }>()(
  persist(set => ({ size: "medium", setSize: size => set({ size }) }), { name: "mutis:chat-pet-size" }),
);
