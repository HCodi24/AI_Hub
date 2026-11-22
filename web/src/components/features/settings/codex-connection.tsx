import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { codexApi, safeCodexLoginUrl } from "@/apis/codex.api";
import { Button } from "@/components/button/button";
import { CliProjectOptions } from "./cli-project-options";
import { settingsText as t } from "@/text/settings";
import { AccountSummary, ReplaceConfirm } from "./account-parts";
import styles from "./settings.module.css";

/** `mode`: a machine source reuses the installed CLI's account, a web source signs in through the browser. */
export function CodexConnection({ providerId, mode }: { providerId: string; mode: "machine" | "web" }) {
  const client = useQueryClient();
  const key = ["codex-connection", providerId];
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => codexApi.status(providerId, signal),
    refetchInterval: query => query.state.data?.pending ? 2000 : false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [asking, setAsking] = useState<"cli" | "web" | null>(null);
  const locked = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const state = query.data;
  useEffect(() => {
    if (state) client.setQueryData(["provider-test", providerId], { ok: state.connected,
      status: state.connected ? "connected" : "not_configured", message: state.message, detail: "" });
  }, [client, providerId, state]);

  async function act(action: "cli" | "web" | "cancel" | "logout" | "sandbox") {
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

  /** A sign-in that would replace the current account waits for confirmation first. */
  function signIn(action: "cli" | "web") {
    if (state?.connected) setAsking(action);
    else void act(action);
  }

  const url = safeCodexLoginUrl(state?.url);
  return <section className={mode === "machine" ? styles.cliMachineGroup : styles.group} aria-label="Kết nối Codex trong MutiS" aria-busy={busy}>
    {mode === "web" && <><h3>Codex trong MutiS</h3>
      <AccountSummary account={state} /></>}
    <div className={styles.actions}>
      {mode === "web" && <Button disabled={busy || Boolean(state?.pending)} onClick={() => signIn("web")}>{state?.connected ? t.providers.account.useWeb : t.providers.account.signInWeb}</Button>}
      {state?.pending && <Button disabled={busy} onClick={() => void act("cancel")}>Hủy đăng nhập</Button>}
      {mode === "web" && state?.connected && <Button disabled={busy} onClick={() => void act("logout")}>Đăng xuất khỏi MutiS</Button>}
      {state?.connected && state.sandbox !== "ready" && state.sandbox !== "unknown" && <Button disabled={busy} onClick={() => void act("sandbox")}>Thiết lập sandbox</Button>}
    </div>
    {asking && state?.connected && <ReplaceConfirm account={state} disabled={busy}
      onConfirm={() => { const action = asking; setAsking(null); void act(action); }} onCancel={() => setAsking(null)} />}
    {state?.connected && state.sandbox !== "ready" && state.sandbox !== "unknown" && <p className={styles.note} role="alert">
      Codex riêng của MutiS chưa có sandbox Windows nên chưa ghi được file (chỉ đọc). Bấm «Thiết lập sandbox» rồi chọn Có ở hộp thoại quyền quản trị của Windows; việc này chỉ làm một lần.</p>}
    {(busy || !state?.connected) && <p className={styles.note} role="status">{busy ? "Đang xử lý…" : state?.message || (query.isPending ? "Đang kiểm tra Codex…" : "Chưa kiểm tra được Codex")}</p>}
    {state?.pending && url && <a href={url} target="_blank" rel="noopener noreferrer">Mở trang đăng nhập</a>}
    <CliProjectOptions state={state} instructionsFile="AGENTS.md" disabled={busy || Boolean(state?.pending)}
      onChange={(key, checked) => void update(key, checked)} />
    <p className={styles.note}>Hai tùy chọn áp dụng cho các nguồn Codex trong MutiS. Thay đổi sẽ tạo ngữ cảnh Codex mới khi chat tiếp; lịch sử và memory của app được giữ.</p>
    {(error || query.error) && <p role="alert">{error || query.error?.message}</p>}
  </section>;
}
