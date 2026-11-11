import { useQuery, useQueryClient } from "@tanstack/react-query";
import { settingsApi } from "@/apis/settings.api";
import type { FullConfig } from "@/schema/settings.schema";
/** Full config (with provider details) for the settings dialog; keeps the chat's lighter ["config"] query in sync. */
export function useSettingsConfig() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["settings-config"], queryFn: ({ signal }) => settingsApi.config(signal) });
  function apply(config: FullConfig) {
    client.setQueryData(["settings-config"], config);
    void client.invalidateQueries({ queryKey: ["config"] });
    void client.invalidateQueries({ queryKey: ["connections"] });
  }
  return { query, apply };
}
export const errorText = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;
