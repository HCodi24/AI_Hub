"use client";

import { useCallback, useEffect, useState } from "react";

/** localStorage key shared by every chat and every same-origin tab. */
export const DASHBOARD_PREFERENCE_KEY = "ai-hub-dashboard-enabled";

/** Stored format is the literal string "true" or "false"; anything else counts as unset. */
function parseStored(raw: string | null): boolean {
  return raw === "true";
}

function readStored(): boolean {
  try {
    if (typeof window === "undefined") return false;
    return parseStored(window.localStorage.getItem(DASHBOARD_PREFERENCE_KEY));
  } catch {
    return false;
  }
}

function writeStored(enabled: boolean): void {
  try {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(DASHBOARD_PREFERENCE_KEY, enabled ? "true" : "false");
  } catch {
    /* storage unavailable: keep the in-memory value only */
  }
}

/**
 * Dashboard visibility preference persisted in localStorage.
 * State starts as `false` so server rendering never touches browser APIs; the saved
 * value is read in a mount effect that never writes, so a saved `true` is not
 * clobbered by the default (including under React StrictMode double effects).
 */
export function useDashboardPreference(): [boolean, (enabled: boolean) => void] {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    setEnabled(readStored());
  }, []);

  // Follow changes made by other same-origin tabs. Storage events never write back.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onStorage = (event: StorageEvent) => {
      if (event.key !== null && event.key !== DASHBOARD_PREFERENCE_KEY) return;
      // `key === null` means localStorage.clear(): reset to the default.
      setEnabled(event.key === null ? false : parseStored(event.newValue));
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const setDashboardEnabled = useCallback((next: boolean) => {
    setEnabled(next);
    writeStored(next);
  }, []);

  return [enabled, setDashboardEnabled];
}
