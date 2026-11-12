import { useRef, useState } from "react";
import { ExternalLink, LogIn } from "lucide-react";
import { safeClaudeLoginUrl } from "@/apis/claude.api";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { settingsText as t } from "@/text/settings";
import { errorText } from "./use-settings";
import styles from "./settings.module.css";
/** Claude Code CLI sign-in: opens the login URL, then the user pastes the confirmation code. */
export function ClaudeLogin({ providerId, onDone, disabled = false, onPending }: { providerId: string; onDone: () => void; disabled?: boolean; onPending?: (value: boolean) => void }) {
  const locked = useRef(false);
  const [session, setSession] = useState<{ session_id: string; url: string } | null>(null);
  const [code, setCode] = useState(""); const [busy, setBusy] = useState(false); const [message, setMessage] = useState(""); const [tail, setTail] = useState("");
  async function run(action: () => Promise<void>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setMessage("");
    try { await action(); } catch (error) { setMessage(errorText(error, t.error)); } finally { locked.current = false; setBusy(false); }
  }
  if (!session) return <><Button disabled={busy || disabled} onClick={() => {
    if (locked.current) return;
    const popup = window.open("about:blank", "_blank");
    if (popup) popup.opener = null;
    void run(async () => {
      onPending?.(true); setCode(""); setTail("");
      try {
        const result = await settingsApi.loginStart(providerId);
        const url = safeClaudeLoginUrl(result.url);
        if (!url) { await settingsApi.loginCancel(providerId); throw new Error("Link đăng nhập Claude không hợp lệ"); }
        setSession({ ...result, url }); onPending?.(true);
        if (popup) popup.location.href = url;
      } catch (error) { popup?.close(); onPending?.(false); throw error; }
    });
  }}><LogIn size={16} aria-hidden="true" />Đăng nhập qua web</Button>
    {message && <p role="alert">{message}</p>}</>;
  return <div className={styles.confirm} role="group" aria-label={t.providers.login}>
    <a href={session.url} target="_blank" rel="noreferrer">{t.providers.loginOpen} <ExternalLink size={14} aria-hidden="true" /></a>
    <label>{t.providers.loginCode}<input value={code} autoComplete="off" spellCheck={false} onChange={event => setCode(event.target.value)} /></label>
    <Button variant="primary" disabled={busy || !code.trim()} onClick={() => void run(async () => {
      const result = await settingsApi.loginCode(providerId, session.session_id, code.trim());
      setMessage(result.message); setTail(result.output_tail);
      if (result.logged_in) { setSession(null); setCode(""); setTail(""); onPending?.(false); notify(t.providers.loginDone); onDone(); }
    })}>{t.providers.loginSubmit}</Button>
    <Button disabled={busy} onClick={() => void run(async () => { await settingsApi.loginCancel(providerId); setSession(null); setCode(""); setTail(""); onPending?.(false); })}>{t.cancel}</Button>
    {message && <p className={styles.note} role="status">{message}</p>}{tail && <pre className={styles.log}>{tail}</pre>}
  </div>;
}
