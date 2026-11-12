import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { codexApi, safeCodexLoginUrl } from "@/apis/codex.api";
import { Button } from "@/components/button/button";
import styles from "./settings.module.css";

export function CodexConnection({ providerId }: { providerId: string }) {
  const client = useQueryClient();
  const key = ["codex-connection", providerId];
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => codexApi.status(providerId, signal),
    refetchInterval: query => query.state.data?.pending ? 2000 : false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const locked = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const state = query.data;
  useEffect(() => {
    if (state) client.setQueryData(["provider-test", providerId], { ok: state.connected,
      status: state.connected ? "connected" : "not_configured", message: state.message, detail: "" });
  }, [client, providerId, state]);

  async function act(action: "cli" | "web" | "cancel" | "logout") {
    if (locked.current) return;
    const popup = action === "web" ? window.open("about:blank", "_blank") : null;
    if (popup) popup.opener = null;
    locked.current = true; setBusy(true); setError("");
    try {
      await client.cancelQueries({ queryKey: key });
      const result = await codexApi.action(providerId, action);
      client.setQueryData(key, result);
      const url = safeCodexLoginUrl(result.url);
      if (action === "web") {
        if (url && popup) popup.location.href = url;
        else if (popup) popup.close();
      }
    } catch (error) {
      popup?.close();
      if (mounted.current) setError(error instanceof Error ? error.message : "Không kết nối được Codex");
      await client.invalidateQueries({ queryKey: ["provider-test", providerId] });
    } finally { locked.current = false; if (mounted.current) setBusy(false); }
  }

  async function update(name: "project_skills" | "project_instructions", value: boolean) {
    if (!state || locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try {
      await client.cancelQueries({ queryKey: key });
      const next = await codexApi.settings(providerId, { project_skills: state.project_skills,
        project_instructions: state.project_instructions, [name]: value });
      client.setQueryData(key, next);
    } catch (error) { if (mounted.current) setError(error instanceof Error ? error.message : "Không lưu được cài đặt"); }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  }

  const url = safeCodexLoginUrl(state?.url);
  return <section className={styles.group} aria-label="Kết nối Codex trong MutiS" aria-busy={busy}>
    <h3>Codex trong MutiS</h3>
    <p className={styles.note}>Dùng môi trường riêng của MutiS. Tìm CLI chỉ dùng lại đăng nhập có sẵn nếu tương thích.</p>
    <div className={styles.actions}>
      <Button disabled={busy || Boolean(state?.pending)} onClick={() => void act("cli")}>Tìm Codex CLI</Button>
      <Button disabled={busy || Boolean(state?.pending)} onClick={() => void act("web")}>Đăng nhập qua web</Button>
      {state?.pending && <Button disabled={busy} onClick={() => void act("cancel")}>Hủy đăng nhập</Button>}
      {state?.connected && <Button disabled={busy} onClick={() => void act("logout")}>Đăng xuất khỏi MutiS</Button>}
    </div>
    <p className={styles.note} role="status">{busy ? "Đang xử lý…" : state?.message || (query.isPending ? "Đang kiểm tra Codex…" : "Chưa kiểm tra được Codex")}</p>
    {state?.pending && url && <a href={url} target="_blank" rel="noopener noreferrer">Mở trang đăng nhập</a>}
    <label className={styles.check}><input type="checkbox" checked={state?.project_skills || false} disabled={!state || busy || state.pending}
      onChange={event => void update("project_skills", event.target.checked)} /><span>Nạp skill của dự án</span></label>
    <label className={styles.check}><input type="checkbox" checked={state?.project_instructions || false} disabled={!state || busy || state.pending}
      onChange={event => void update("project_instructions", event.target.checked)} /><span>Nạp hướng dẫn dự án (AGENTS.md)</span></label>
    <p className={styles.note}>Hai tùy chọn áp dụng cho các nguồn Codex trong MutiS. Thay đổi sẽ tạo ngữ cảnh Codex mới khi chat tiếp; lịch sử và memory của app được giữ.</p>
    {(error || query.error) && <p role="alert">{error || query.error?.message}</p>}
  </section>;
}
