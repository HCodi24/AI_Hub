import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { claudeApi, type ClaudeStatus } from "@/apis/claude.api";
import { Button } from "@/components/button/button";
import { ClaudeLogin } from "./claude-login";
import styles from "./settings.module.css";

export function ClaudeConnection({ providerId }: { providerId: string }) {
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
  return <section className={styles.group} aria-label="Kết nối Claude trong MutiS" aria-busy={busy || loginBusy}>
    <h3>Claude trong MutiS</h3>
    <p className={styles.note}>Cấu hình riêng cho nguồn Claude này. Chỉ dùng lại xác thực CLI dạng file tương thích; không nhập cấu hình cá nhân.</p>
    <div className={styles.actions}>
      <Button disabled={disabled} onClick={() => void run(() => claudeApi.action(providerId, "cli"))}>Tìm Claude CLI</Button>
      {state?.connected && <Button disabled={disabled} onClick={() => void run(() => claudeApi.action(providerId, "logout"))}>Đăng xuất khỏi MutiS</Button>}
    </div>
    <ClaudeLogin providerId={providerId} disabled={busy} onPending={value => { setLoginBusy(value); if (!value) refresh(); }} onDone={refresh} />
    <p role="status" className={styles.note}>{busy || loginBusy ? "Đang xử lý…" : state?.message || (query.isPending ? "Đang kiểm tra Claude…" : "Chưa kiểm tra được Claude")}</p>
    <label className={styles.check}><input type="checkbox" checked={state?.project_skills || false} disabled={!state || disabled}
      onChange={event => { const checked = event.target.checked; if (state) void run(() => claudeApi.settings(providerId, { ...state, project_skills: checked })); }} /><span>Nạp skill của dự án</span></label>
    <label className={styles.check}><input type="checkbox" checked={state?.project_instructions || false} disabled={!state || disabled}
      onChange={event => { const checked = event.target.checked; if (state) void run(() => claudeApi.settings(providerId, { ...state, project_instructions: checked })); }} /><span>Nạp hướng dẫn dự án (CLAUDE.md)</span></label>
    <p className={styles.note}>Áp dụng cho các nguồn Claude. Hướng dẫn gồm CLAUDE.md, CLAUDE.local.md và .claude/rules. Skill được cấp dưới dạng nội dung SKILL.md. Thay đổi tạo phiên mới khi chat tiếp; lịch sử và memory MutiS được giữ.</p>
    {(error || query.error) && <p role="alert">{error || query.error?.message}</p>}
  </section>;
}
