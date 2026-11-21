import { useState } from "react";
import { Button } from "@/components/button/button";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { notify } from "@/components/toast/toast";
import { canUpdate, updateApi, type AppUpdate } from "@/lib/desktop-update";
import { settingsText } from "@/text/settings";
import styles from "./settings.module.css";

const t = settingsText.update;
/** Settings › Chung › Cập nhật ứng dụng: check for a newer version and install it (desktop app only). */
export function UpdateSection() {
  const [state, setState] = useState<"idle" | "checking" | "installing">("idle");
  const [update, setUpdate] = useState<AppUpdate | null | undefined>(undefined);
  // Installing stops the server and the Codex / Claude processes under it, so never while something is running.
  const running = useChatStore(s => s.streaming) || useCardStore(s => s.running !== null);
  const fail = (error: unknown) => { setState("idle"); notify(`${t.error}: ${error instanceof Error ? error.message : String(error)}`); };
  const check = () => { setState("checking"); updateApi.check().then(found => { setUpdate(found); setState("idle"); }, fail); };
  const install = () => { setState("installing"); updateApi.install().catch(fail); };
  return <fieldset className={styles.group}><legend>{t.title}</legend>
    {!canUpdate() ? <p>{t.desktopOnly}</p> : <>
      {update === null && <p>{t.latest}</p>}
      {update && <p><strong>{t.available(update.version)}</strong>{update.notes && <><br />{t.notes}: {update.notes}</>}</p>}
      {update && running && <p role="status">{t.busy}</p>}
      {state === "installing" && <p role="status">{t.installing}</p>}
      <div className={styles.actions}>
        {update
          ? <Button type="button" variant="primary" disabled={state !== "idle" || running} onClick={install}>{t.install}</Button>
          : <Button type="button" disabled={state !== "idle"} onClick={check}>{state === "checking" ? t.checking : t.check}</Button>}
      </div></>}
  </fieldset>;
}
