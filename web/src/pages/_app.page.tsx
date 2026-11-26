import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { AppProps } from "next/app";
import { useState } from "react";
import { TooltipLayer } from "@/components/tooltip/tooltip-layer";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@/styles/globals.css";
import "@/components/features/settings/appearance/theme/glass/index.css";
export default function App({ Component, pageProps }: AppProps) {
  const [queryClient] = useState(() => new QueryClient({ defaultOptions: {
    queries: { refetchOnWindowFocus: false, retry: false, staleTime: 30_000 },
  } }));
  return (
    <QueryClientProvider client={queryClient}>
      <Component {...pageProps} />
      <TooltipLayer />
    </QueryClientProvider>
  );
}
