import { useQueries } from "@tanstack/react-query";
import { settingsApi } from "@/apis/settings.api";

// Chat and Settings share results; opening a menu never starts a connection test.
export function useProviderConnections(providers: readonly { id: string }[]) {
  const tests = useQueries({ queries: providers.map(provider => ({
    queryKey: ["provider-test", provider.id],
    queryFn: () => settingsApi.testProvider(provider.id),
    staleTime: Infinity,
    gcTime: Infinity,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })) });
  return Object.fromEntries(providers.map((provider, index) => [provider.id, tests[index]?.data]));
}
