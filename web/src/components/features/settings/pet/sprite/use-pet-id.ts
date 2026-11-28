import { useQuery } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { petForSlot, petKind } from "@/lib/pet-choice";

/** Id of the pet the user chose for this source and model; undefined when none is chosen or pets are switched off. */
export function usePetId(provider: string, model = ""): string | undefined {
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data;
  return config && config.pets_enabled ? petForSlot(config.pets, petKind(provider, model)) : undefined;
}
