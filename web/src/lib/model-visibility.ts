import { create } from "zustand";
import { persist } from "zustand/middleware";

type ModelVisibility = {
  hiddenModels: string[];
  setModelVisible: (providerId: string, model: string, visible: boolean) => void;
};

const modelKey = (providerId: string, model: string) => JSON.stringify([providerId, model]);

export function isModelVisible(hiddenModels: readonly string[], providerId: string, model: string) {
  return !hiddenModels.includes(modelKey(providerId, model));
}

export const useModelVisibility = create<ModelVisibility>()(persist((set) => ({
  hiddenModels: [],
  setModelVisible: (providerId, model, visible) => set(({ hiddenModels }) => {
    const key = modelKey(providerId, model);
    if (visible === !hiddenModels.includes(key)) return { hiddenModels };
    return { hiddenModels: visible ? hiddenModels.filter(item => item !== key) : [...hiddenModels, key] };
  }),
}), { name: "mutis:model-visibility" }));
