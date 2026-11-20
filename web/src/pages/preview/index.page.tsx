"use client";

import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { DashboardDialog } from "@/components/features/chat/dashboard-dialog";
import { useDashboardPreference } from "@/components/features/chat/use-dashboard-preference";
import { useTaskBoardPreference } from "@/components/features/chat/use-task-board-preference";
import { SettingsDialog, isTabId } from "@/components/features/settings/settings-dialog";

import { installPreviewMock } from "@/lib/preview-mock";

const noop = () => {};
const THEMES = ["light", "dark", "system"];
// Installed when the module loads, before any child fetches. `?live=1` keeps the real server for writes too.
if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("live") !== "1") installPreviewMock();

/**
 * Renders one screen of the app on its own, for the preview panel: `/app/preview/?c=settings&tab=extensions`.
 * Add a screen by adding a branch below (its dialog is opened at once and its close button does nothing).
 */
export default function PreviewPage() {
  const [query, setQuery] = useState<URLSearchParams | null>(null);
  const [dashboardEnabled, setDashboardEnabled] = useDashboardPreference();
  const [taskBoardEnabled, setTaskBoardEnabled] = useTaskBoardPreference();
  // The query string only exists in the browser (static export), so it is read after mount.
  useEffect(() => setQuery(new URLSearchParams(window.location.search)), []);
  // The dev server runs on another origin, so the app's cached theme is missing here: use `?theme=` or the theme saved in the server config.
  const savedTheme = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data?.theme;
  const theme = query?.get("theme") || savedTheme;
  useEffect(() => { if (theme && THEMES.includes(theme)) document.documentElement.dataset.theme = theme; }, [theme]);
  if (!query) return null;
  const screen = query.get("c");
  const tab = query.get("tab");
  if (screen === "settings") return <SettingsDialog initialTab={isTabId(tab) ? tab : undefined} folder={query.get("folder") ?? ""} close={noop}
    dashboardEnabled={dashboardEnabled} onDashboardEnabledChange={setDashboardEnabled}
    taskBoardEnabled={taskBoardEnabled} onTaskBoardEnabledChange={setTaskBoardEnabled} />;
  if (screen === "dashboard") return <DashboardDialog close={noop} />;
  return <p style={{ padding: 16 }}>Preview: c=settings | dashboard</p>;
}
