"use client";

import { useCallback, useEffect, useState } from "react";

/** localStorage key shared by every chat and every same-origin tab. */
export const TASK_BOARD_PREFERENCE_KEY = "ai-hub-task-board-enabled";

/** The board is on unless the stored value is the literal string "false". */
const parseStored = (raw: string | null) => raw !== "false";

function readStored(): boolean {
  try { return typeof window === "undefined" ? true : parseStored(window.localStorage.getItem(TASK_BOARD_PREFERENCE_KEY)); }
  catch { return true; }
}

/** Whether the live "Tiến độ các task" board is shown; persisted in localStorage, on by default. */
export function useTaskBoardPreference(): [boolean, (enabled: boolean) => void] {
  const [enabled, setEnabled] = useState(true);
  useEffect(() => { setEnabled(readStored()); }, []);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onStorage = (event: StorageEvent) => {
      if (event.key !== null && event.key !== TASK_BOARD_PREFERENCE_KEY) return;
      setEnabled(event.key === null ? true : parseStored(event.newValue));
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);
  const set = useCallback((next: boolean) => {
    setEnabled(next);
    try { window.localStorage.setItem(TASK_BOARD_PREFERENCE_KEY, next ? "true" : "false"); }
    catch { /* storage unavailable: keep the in-memory value only */ }
  }, []);
  return [enabled, set];
}
