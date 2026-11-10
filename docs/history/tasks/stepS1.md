Step S1 of AI Hub — chat run status + hang protection. FIRST read docs/CODEX_RULES.md and docs/SPEC.md.
Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1. No real AI calls.
NOTE: the user runs the app in dev mode from this repo and a half-finished UI attempt exists (uncommitted diff in
ui/js/chat.js, ui/index.html, ui/styles/mutis.css, src/providers/mod.rs). Build on it, clean it up, do not duplicate it:
`#running-model` badge in the composer + `message.running` indicator + `runningLabel()` (just added in chat.js).
In src/providers/mod.rs the new `strip_verbatim_prefix` duplicates `crate::core::text::strip_verbatim` — delete it and
use the core one. Never stop/kill ai-hub processes; never run `cargo build --release` outside target/codex.

User complaints: (1) while a chat runs nothing shows WHICH AI is running or that it is alive; (2) it sometimes hangs.
Root causes found by the reviewer:
 a) claude_cli.rs: for `result` events only `is_error == true` is reported. A `result` with another subtype
    (e.g. `error_max_turns`, `error_during_execution`, `error_max_budget_usd`) or an empty turn ends the chat with an empty
    bubble and NO error (seen twice).
 b) Claude's text arrives only when a whole assistant message completes (no partial deltas), so between tool calls the UI
    receives nothing for a long time and looks frozen.
 c) The only idle watchdog is Codex-only and only when MutiS is enabled (`watchdog_minutes`). Claude CLI, API providers and
    deepseek_agent have no idle/overall timeout.

Implement:
1. Server `status` + heartbeat events on /api/chat SSE (shared in the chat run loop, provider-agnostic):
   - first event `status` {provider_name, model, effort, role?, started_at_ms};
   - every 3 s a `heartbeat` {elapsed_s, idle_s, last_activity} where idle_s = seconds since the last event received from
     the provider and last_activity = short text of the last `log` (e.g. "Đọc ui/js/chat.js", "Sửa ...").
   Heartbeats must not be persisted in the chat file and must stop when the run ends/cancels.
2. UI (ui/js/chat.js, styles/chat.css): while streaming show a status strip above the composer (replace/finish the
   `#running-model` badge): animated dot + "<provider> · <model> · <effort>" + elapsed "mm:ss" + "hoạt động cuối: <last_activity>
   (<idle_s>s trước)". When idle_s ≥ 45 show a warning style "⚠ Chưa có phản hồi 45s — AI có thể đang suy nghĩ lâu hoặc bị
   treo" with the existing Dừng button emphasised; ≥ 120 s add "Bấm Dừng rồi gửi lại nếu cần". Also put the live
   `.running-indicator` ("<provider> đang chạy…" with animated dots) at the end of the streaming assistant message (clear it when
   done). In MutiS mode / plan task runs show the acting role + provider too.
3. Fix (a): in claude_cli.rs handle every `result` event: if subtype != "success" or is_error → emit an `error` with a
   Vietnamese explanation per subtype (error_max_turns → "Claude dừng vì hết số lượt (max turns)", error_during_execution →
   "Claude gặp lỗi khi chạy", else the subtype name) and include `result` text if present. If the turn finished successfully
   but produced no `delta` at all and `result.result` is non-empty, emit it as a `delta`. If the process ended with NO `result`
   event, emit error "Claude Code CLI kết thúc đột ngột (không có kết quả)".
   Generic: in the chat run loop, if a run ends with no delta, no error and no cancel → emit `error`
   "AI kết thúc mà không trả lời — có thể bị ngắt giữa chừng. Hãy gửi lại." (UI shows it in the bubble).
4. Fix (b): add `--include-partial-messages` to the Claude stream-json args and map `stream_event` deltas
   (`content_block_delta` with `text_delta`) to `delta` events; do NOT then also emit the full assistant text (avoid
   duplicates — emit full `text` blocks only if no partial deltas were received for that message). Keep tool_use logs.
5. Fix (c): idle watchdog for ALL providers/runs, independent of MutiS: new config `chat_idle_timeout_minutes`
   (default 10, 0 = off, range 0–120) in the main config (Settings › a short "Chat" section; also readable via /api/config).
   If no provider event arrives for that long: kill the child tree / abort the HTTP stream and emit error
   "Không có phản hồi sau N phút — đã dừng" + event kind `watchdog_kill` when MutiS event log is on. The existing Codex
   MutiS watchdog keeps working (the shorter wins).
6. Claude `rate_limit_event`: currently logged as a raw line. If its info shows status other than "allowed"/ok or has a
   resetsAt, emit a readable log "Claude đang gần/chạm giới hạn lượt dùng — reset lúc <time>"; otherwise drop the event from
   the logs (too noisy).
7. Connection loss: in chat.js if the fetch/stream rejects or ends without a `done` event, show in the bubble
   "Mất kết nối với app (có thể app đang khởi động lại). Tin nhắn có thể chưa hoàn tất — kiểm tra rồi gửi lại." and always
   leave streaming state (already in finally — verify it also clears the status strip and timers).
8. Unit tests: result-subtype mapping, "no delta / no result" detection (pure functions), idle timeout decision, partial-delta
   dedupe. Update README (Chat) + docs/SPEC.md (SSE events status/heartbeat, new setting).
Reply per docs/CODEX_RULES.md and list anything not done.
