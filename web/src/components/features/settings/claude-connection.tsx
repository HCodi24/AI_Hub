import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { claudeApi, type ClaudeStatus } from "@/apis/claude.api";
import { Button } from "@/components/button/button";
import { CliProjectOptions } from "./cli-project-options";
import { AccountSummary } from "./account-parts";
import { ClaudeLogin } from "./claude-login";
import styles from "./settings.module.css";

/** `mode`: a machine source reuses the installed CLI's account, a web source signs in through the browser. */
export function ClaudeConnection({ providerId, mode }: { providerId: string; mode: "machine" | "web" }) {
  const client = useQueryClient();
  const key = ["claude-connection", providerId];
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => claudeApi.status(providerId, signal) });
  const [busy, setBusy] = useState(false);
  const [loginBusy, setLoginBusy] = useState(false);
  const [error, setError] = useState("");
  const locked = useRef(false);
  const state = query.data;
  const disabled = busy || loginBusy || Boolean(state?.pending);
  useEffect(() => {
    if (state) client.setQueryData(["provider-test", providerId], { ok: state.connected,
      status: state.connected ? "connected" : "not_configured", message: state.message, detail: "" });
  }, [client, providerId, state]);
  async function run(action: () => Promise<ClaudeStatus>) {
    if (locked.current || loginBusy) return;
    locked.current = true; setBusy(true); setError("");
    try {
      await client.cancelQueries({ queryKey: key });
      client.setQueryData(key, await action());
    } catch (error) { setError(error instanceof Error ? error.message : "Không xử lý được kết nối Claude"); }
    finally { locked.current = false; setBusy(false); }
  }
  const refresh = () => { void client.invalidateQueries({ queryKey: key }); void client.invalidateQueries({ queryKey: ["provider-test", providerId] }); };
  return <section className={mode === "machine" ? styles.cliMachineGroup : styles.group} aria-label="Kết nối Claude trong MutiS" aria-busy={busy || loginBusy}>
    {mode === "web" && <>
      <h3>Claude trong MutiS</h3>
      <AccountSummary account={state} />
      {state?.connected && <div className={styles.actions}>
        <Button disabled={disabled} onClick={() => void run(() => claudeApi.action(providerId, "logout"))}>Đăng xuất khỏi MutiS</Button>
      </div>}
    </>}
    {mode === "web" && <ClaudeLogin providerId={providerId} disabled={busy} replacing={state?.connected ? state : undefined}
      onPending={value => { setLoginBusy(value); if (!value) refresh(); }} onDone={refresh} />}
    {(busy || loginBusy || !state?.connected) && <p role="status" className={styles.note}>{busy || loginBusy ? "Đang xử lý…" : state?.message || (query.isPending ? "Đang kiểm tra Claude…" : "Chưa kiểm tra được Claude")}</p>}
    <CliProjectOptions state={state} instructionsFile="CLAUDE.md" disabled={disabled}
      onChange={(key, checked) => { if (state) void run(() => claudeApi.settings(providerId, { ...state, [key]: checked })); }} />
    <p className={styles.note}>Áp dụng cho các nguồn Claude. Hướng dẫn gồm CLAUDE.md, CLAUDE.local.md và .claude/rules. Skill được cấp dưới dạng nội dung SKILL.md. Thay đổi tạo phiên mới khi chat tiếp; lịch sử và memory MutiS được giữ.</p>
    {(error || query.error) && <p role="alert">{error || query.error?.message}</p>}
  </section>;
}
