import { useQueries } from "@tanstack/react-query";
import { settingsApi } from "@/apis/settings.api";

// Chat and Settings share results; opening a menu never starts a connection test.
export function useProviderConnections(providers: readonly { id: string }[]) {
  const tests = useProviderConnectionQueries(providers);
  return Object.fromEntries(providers.map((provider, index) => [provider.id, tests[index]?.data]));
}

export function useProviderConnectionQueries<T extends { id: string }>(
  providers: readonly T[],
  refresh?: (provider: T) => boolean,
) {
  return useQueries({ queries: providers.map(provider => ({
    queryKey: ["provider-test", provider.id],
    queryFn: () => settingsApi.testProvider(provider.id),
    staleTime: Infinity,
    gcTime: Infinity,
    retry: false,
    refetchOnMount: refresh?.(provider) ? "always" as const : undefined,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })) });
}
