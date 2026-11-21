import { notify } from "@/components/toast/toast";
import { settingsText } from "@/text/settings";

export type AppUpdate = { version: string; notes: string };
type Tauri = { core: { invoke<T>(command: string): Promise<T> } };
const tauri = () => (window as unknown as { __TAURI__?: Tauri }).__TAURI__;
const CHECKED_KEY = "ai-hub-update-checked";
const DAY = 24 * 60 * 60 * 1000;

/** Updates exist only in the desktop window; in a plain browser the commands are missing. */
export const canUpdate = () => Boolean(tauri());
export const updateApi = {
  /** The newest version, or null when this one is it. */
  check: () => tauri()!.core.invoke<AppUpdate | null>("update_check"),
  /** Downloads and installs; the app closes and the installer starts it again. */
  install: () => tauri()!.core.invoke<void>("update_install"),
};

/** At most once a day, silently: a toast only when a newer version exists. Errors (offline, no release yet) stay quiet. */
export async function notifyIfUpdate() {
  if (!canUpdate()) return;
  try {
    if (Date.now() - Number(localStorage.getItem(CHECKED_KEY) ?? 0) < DAY) return;
    localStorage.setItem(CHECKED_KEY, String(Date.now()));
  } catch { /* storage unavailable: check on every start */ }
  const update = await updateApi.check().catch(() => null);
  if (update) notify(settingsText.update.toast(update.version));
}
