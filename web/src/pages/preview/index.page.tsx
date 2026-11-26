"use client";

import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { ChatScreen } from "@/components/features/chat/chat-screen";
import { DashboardDialog } from "@/components/features/chat/dashboard/dashboard-dialog";
import { TaskBoardView } from "@/components/features/chat/task_board/task-board";
import { PreviewPanel } from "@/components/features/preview/preview-panel";
import type { TaskRow } from "@/schema/chat.schema";
import { useDashboardPreference } from "@/components/features/chat/dashboard/use-dashboard-preference";
import { useTaskBoardPreference } from "@/components/features/chat/task_board/use-task-board-preference";
import { SettingsDialog, isTabId } from "@/components/features/settings/settings-dialog";

import { installPreviewMock, installReadOnlyPreview } from "@/lib/preview-mock";

const noop = () => {};
// One row per state the board can show; `?c=tasks&mock=many` repeats them.
const TASK_STATES = ["waiting", "coding", "checking", "fixing", "done", "failed", "limited", "takeover", "skipped", "stopped"];
const SAMPLE_TASKS: TaskRow[] = TASK_STATES.map((state, i) => ({
  title: `Việc mẫu ${i + 1}`,
  state,
  attempt: state === "takeover" ? 3 : i % 3 + 1,
  check: state === "done" ? "PASS" : null,
  reason: state === "takeover" ? "vẫn FAIL sau 3/3 lần" : state === "skipped" ? "phụ thuộc «Việc mẫu 1» chưa đạt" : null,
}));
const THEMES = ["light", "dark", "system"];
const STYLES = ["default", "glass"];
const isProvidersPreview = (query: URLSearchParams) => {
  const tab = query.get("tab");
  return query.get("c") === "settings" && (!isTabId(tab) || tab === "providers");
};

// Install the fetch guard before any preview queries start.
if (typeof window !== "undefined") {
  const query = new URLSearchParams(window.location.search);
  if (isProvidersPreview(query)) installReadOnlyPreview();
  else if (query.get("live") !== "1") installPreviewMock();
}

/**
 * Renders one screen of the app on its own, for the preview panel: `/app/preview/?c=settings&tab=extensions`.
 * Screens: settings, dashboard, chat (the whole app), tasks (task board), preview (preview panel). `&mock=empty|many|error` reshapes the data.
 * Add a screen by adding a branch below (its dialog is opened at once and its close button does nothing).
 */
export default function PreviewPage() {
  const [query, setQuery] = useState<URLSearchParams | null>(null);
  const [dashboardEnabled, setDashboardEnabled] = useDashboardPreference();
  const [taskBoardEnabled, setTaskBoardEnabled] = useTaskBoardPreference();
  // The query string only exists in the browser (static export), so it is read after mount.
  useEffect(() => setQuery(new URLSearchParams(window.location.search)), []);
  // Khi chạy khác nguồn, trang xem trước lấy giao diện từ URL hoặc cấu hình máy chủ.
  const { data } = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const theme = query?.get("theme") || data?.theme;
  const requestedStyle = query?.get("style");
  const style = requestedStyle && STYLES.includes(requestedStyle) ? requestedStyle : data?.style;
  useEffect(() => { if (theme && THEMES.includes(theme)) document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => { if (style && STYLES.includes(style)) document.documentElement.dataset.style = style; }, [style]);
  if (!query) return null;
  const screen = query.get("c");
  const tab = query.get("tab");
  if (screen === "settings") return <SettingsDialog initialTab={isTabId(tab) ? tab : undefined} folder={query.get("folder") ?? ""} close={noop}
    dashboardEnabled={dashboardEnabled} onDashboardEnabledChange={setDashboardEnabled}
    taskBoardEnabled={taskBoardEnabled} onTaskBoardEnabledChange={setTaskBoardEnabled} />;
  if (screen === "dashboard") return <DashboardDialog close={noop} />;
  if (screen === "chat") return <ChatScreen />;
  if (screen === "tasks") return <div style={{ padding: 16, maxWidth: 640 }}><TaskBoardView tasks={query.get("mock") === "empty" ? [] : query.get("mock") === "many" ? [...SAMPLE_TASKS, ...SAMPLE_TASKS.map(task => ({ ...task, title: `${task.title} b` }))] : SAMPLE_TASKS} /></div>;
  if (screen === "preview") return <PreviewPanel />;
  return <p style={{ padding: 16 }}>Preview: c=settings | dashboard | chat | tasks | preview (&amp;mock=empty|many|error)</p>;
}
