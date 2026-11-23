import { useEffect, useState } from "react";
import { Button } from "@/components/button/button";
import { devApi } from "@/apis/dev.api";
import { settingsText } from "@/text/settings";
import styles from "./settings.module.css";

const t = settingsText.dev;
/** Settings › Phát triển › Build bản cài: bumps the last version number, then runs the desktop build. Only when MutiS runs with --dev. */
export function DesktopBuild() {
  const [dev, setDev] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [log, setLog] = useState("");
  const [password, setPassword] = useState("");
  useEffect(() => { devApi.status().then(status => setDev(status.dev), () => setDev(false)); }, []);
  if (!dev) return null;
  async function run(action: "build" | "check" | "publish") {
    setBusy(true); setLog("");
    setMessage({ build: t.desktopBuilding, check: t.desktopChecking, publish: t.desktopPublishing }[action]);
    try {
      await devApi.run("desktop-build", new AbortController().signal, (event, data) => {
        const value = typeof data === "object" && data !== null ? data as Record<string, unknown> : {};
        if (event === "log") setLog(current => `${current}${String(value.text ?? "")}\n`.slice(-12000));
        if (event === "summary") {
          const version = String(value.version);
          const success = { build: t.desktopDone(version, String(value.dir)), check: t.desktopChecked(version), publish: t.desktopPublished(version) };
          const failure = { build: t.desktopFail, check: t.desktopCheckFail, publish: t.desktopPublishFail };
          setMessage((value.ok ? success : failure)[action]);
        }
      }, { action, ...(action === "build" ? { password } : {}) });
    } catch (error) { setMessage(error instanceof Error ? error.message : settingsText.error); } finally { setBusy(false); }
  }
  return <fieldset className={styles.group}><legend>{t.buildTools}</legend>
    <label>{t.signingPassword} <input type="password" autoComplete="off" value={password} onChange={event => setPassword(event.target.value)} /></label>
    <div className={styles.actions}>
      <Button type="button" disabled={busy} onClick={() => void run("build")}>{t.desktopBuild}</Button>
      <Button type="button" disabled={busy} onClick={() => void run("check")}>{t.desktopCheck}</Button>
      <Button type="button" disabled={busy} onClick={() => void run("publish")}>{t.desktopPublish}</Button>
    </div>
    <p className={styles.note}>{t.desktopPublishHint}</p>
    {message && <p role="status">{message}</p>}
    {log && <pre>{log}</pre>}
  </fieldset>;
}
