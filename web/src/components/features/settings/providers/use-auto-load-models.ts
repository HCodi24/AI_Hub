import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { settingsApi } from "@/apis/settings.api";
import { notify } from "@/components/toast/toast";
import { settingsText as t } from "@/text/settings";
import { errorText } from "../use-settings";

type SourceState = { connected: boolean; email?: string | null };
type SourceEntry = { id: string; connected: boolean | undefined; email?: string | null };

export function useAutoLoadModels(entries: readonly SourceEntry[]) {
  const client = useQueryClient();
  const lastKnown = useRef(new Map<string, SourceState>());
  const inFlight = useRef(new Set<string>());

  useEffect(() => {
    async function loadModels(id: string) {
      if (inFlight.current.has(id)) return;
      inFlight.current.add(id);
      try {
        const { models } = await settingsApi.models(id);
        await Promise.all([
          client.invalidateQueries({ queryKey: ["settings-config"] }),
          client.invalidateQueries({ queryKey: ["config"] }),
        ]);
        notify(t.providers.autoLoadedModels(models.length));
      } catch (error) {
        const message = t.providers.autoLoadModelsFailed;
        const detail = errorText(error, "");
        notify(detail ? `${message} (${detail})` : message);
      } finally {
        inFlight.current.delete(id);
      }
    }

    for (const { id, connected, email } of entries) {
      if (connected === undefined) continue;
      const previous = lastKnown.current.get(id);
      lastKnown.current.set(id, { connected, email });

      if (!previous || !connected) continue;
      const signedIn = previous.connected === false;
      const switchedAccount = previous.connected && Boolean(email) && email !== previous.email;
      if (signedIn || switchedAccount) void loadModels(id);
    }
  }, [client, entries]);
}
