import { useState } from "react";
import { ExternalLink, LogIn } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
/** Claude Code CLI sign-in: opens the login URL, then the user pastes the confirmation code. */
export function ClaudeLogin({ providerId, onDone }: { providerId: string; onDone: () => void }) {
  const [session, setSession] = useState<{ session_id: string; url: string } | null>(null);
  const [code, setCode] = useState(""); const [busy, setBusy] = useState(false); const [message, setMessage] = useState(""); const [tail, setTail] = useState("");
  async function run(action: () => Promise<void>) {
    setBusy(true); setMessage("");
    try { await action(); } catch (error) { setMessage(errorText(error, t.error)); } finally { setBusy(false); }
  }
  if (!session) return <Button disabled={busy} onClick={() => void run(async () => setSession(await settingsApi.loginStart(providerId)))}>
    <LogIn size={16} aria-hidden="true" />{t.providers.login}</Button>;
  return <div className={styles.confirm} role="group" aria-label={t.providers.login}>
    <a href={session.url} target="_blank" rel="noreferrer">{t.providers.loginOpen} <ExternalLink size={14} aria-hidden="true" /></a>
    <label>{t.providers.loginCode}<input value={code} autoComplete="off" spellCheck={false} onChange={event => setCode(event.target.value)} /></label>
    <Button variant="primary" disabled={busy || !code.trim()} onClick={() => void run(async () => {
      const result = await settingsApi.loginCode(providerId, session.session_id, code.trim());
      setMessage(result.message); setTail(result.output_tail);
      if (result.logged_in) { setSession(null); notify(t.providers.loginDone); onDone(); }
    })}>{t.providers.loginSubmit}</Button>
    <Button disabled={busy} onClick={() => void run(async () => { await settingsApi.loginCancel(providerId).catch(() => undefined); setSession(null); })}>{t.cancel}</Button>
    {message && <p className={styles.note} role="status">{message}</p>}{tail && <pre className={styles.log}>{tail}</pre>}
  </div>;
}
