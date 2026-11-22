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
  async function build() {
    setBusy(true); setLog(""); setMessage(t.desktopBuilding);
    try {
      await devApi.run("desktop-build", new AbortController().signal, (event, data) => {
        const value = typeof data === "object" && data !== null ? data as Record<string, unknown> : {};
        if (event === "log") setLog(current => `${current}${String(value.text ?? "")}\n`.slice(-12000));
        if (event === "summary") setMessage(value.ok ? t.desktopDone(String(value.version), String(value.dir)) : t.desktopFail);
      }, { password });
    } catch (error) { setMessage(error instanceof Error ? error.message : settingsText.error); } finally { setBusy(false); }
  }
  return <fieldset className={styles.group}><legend>{t.buildTools}</legend>
    <label>{t.signingPassword} <input type="password" autoComplete="off" value={password} onChange={event => setPassword(event.target.value)} /></label>
    <div className={styles.actions}><Button type="button" disabled={busy} onClick={() => void build()}>{t.desktopBuild}</Button></div>
    {message && <p role="status">{message}</p>}
    {log && <pre>{log}</pre>}
  </fieldset>;
}
