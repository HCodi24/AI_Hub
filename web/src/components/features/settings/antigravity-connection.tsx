import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { AccountSummary } from "./account-parts";
import { ClaudeLogin } from "./claude-login";
import { CliProjectOptions } from "./cli-project-options";
import { errorText, useSettingsConfig } from "./use-settings";
import { settingsText as t } from "@/text/settings";
import styles from "./settings.module.css";

export function AntigravityProjectOptions() {
  const { query, apply } = useSettingsConfig();
  const save = useMutation({
    mutationFn: (options: { project_skills?: boolean; project_instructions?: boolean }) =>
      settingsApi.saveConfig({ antigravity: options }),
    onSuccess: apply,
  });
  const error = save.error || query.error;
  return <section className={styles.cliMachineGroup} aria-label="Tuỳ chọn dự án Antigravity" aria-busy={save.isPending}>
    <CliProjectOptions state={query.data?.antigravity} instructionsFile="AGENTS.md, GEMINI.md" disabled={save.isPending}
      onChange={(key, checked) => save.mutate({ [key]: checked })} />
    <p className={styles.note}>Hai tuỳ chọn điều khiển ngữ cảnh dự án do MutiS thêm. Antigravity CLI vẫn tự dò tệp dự án theo cấu hình của nó.</p>
    {error && <p role="alert">{errorText(error, t.error)}</p>}
  </section>;
}

/** Antigravity source signed in through the web: its own Google account, apart from the machine's `agy`.
 *  The sign-in form is the same as Claude's (link, then the confirmation code). */
export function AntigravityConnection({ providerId }: { providerId: string }) {
  const client = useQueryClient();
  const key = ["provider-test", providerId];
  const query = useQuery({ queryKey: key, queryFn: () => settingsApi.testProvider(providerId) });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const state = query.data;
  const refresh = () => { void client.invalidateQueries({ queryKey: key }); };
  async function signOut() {
    setBusy(true); setError("");
    try { client.setQueryData(key, await settingsApi.antigravityAction(providerId, "logout")); }
    catch (failure) { setError(errorText(failure, t.error)); }
    finally { setBusy(false); }
  }
  return <section className={styles.group} aria-label="Kết nối Antigravity trong MutiS" aria-busy={busy}>
    <h3>Antigravity trong MutiS</h3>
    <AccountSummary account={state ? { connected: state.ok, email: state.email, plan: state.plan, origin: "web" } : undefined} />
    <div className={styles.actions}>
      {state?.ok && <Button disabled={busy} onClick={() => void signOut()}>Đăng xuất khỏi MutiS</Button>}
    </div>
    <ClaudeLogin providerId={providerId} disabled={busy} onPending={value => { if (!value) refresh(); }} onDone={refresh}
      replacing={state?.ok ? { connected: true, email: state.email, plan: state.plan, origin: "web" } : undefined} />
    {!state?.ok && <p role="status" className={styles.note}>{query.isPending ? "Đang kiểm tra Antigravity…" : state?.message || "Chưa đăng nhập"}</p>}
    {(error || query.error) && <p role="alert">{error || query.error?.message}</p>}
  </section>;
}
