Step 10a of AI Hub. FIRST read .tasks/RULES.md and follow it. Design: plans/orchestration.md (section "Bật / tắt" and items 1, 2, 7).

1. Config: add `orchestration` to Config (serde default = all OFF except logging ON when master is turned on):
   ```
   orchestration: {
     enabled: bool,                       // master switch
     retry_api: bool, retry_attempts: u32 (3), retry_base_ms: u64 (2000),
     codex_watchdog: bool, watchdog_minutes: u32 (5), codex_auto_resume: bool, resume_max: u32 (2),
     deepseek_agent: bool, deepseek_max_file_lines: u32 (400),
     verify_gate: bool, verify_command: String (empty = auto-detect),
     auto_review: bool, reviewer_provider_id: String, reviewer_model: String,
     auto_fix: bool, fix_rounds_max: u32 (2),
     event_log: bool,
     routing: Vec<RoutingRule>  // {kind:"small_edit|code|test|docs|plan|review", assignee_kind:"deepseek_agent|codex_cli|claude_cli|openai_compat|anthropic", note}
   }
   ```
   Default routing: small_edit→deepseek_agent, docs→deepseek_agent, code→codex_cli, test→codex_cli,
   plan→claude_cli, review→claude_cli. A helper `effective(cfg, flag)` = enabled && flag.
   API: GET/PUT `/api/orchestration` (validate ranges: attempts 1–5, watchdog 1–60, resume 0–3, fix rounds 0–5,
   max_file_lines 50–5000). Sub-features only take effect when the master switch is on.
2. UI: Settings gets a "Điều phối" section: master toggle "Điều phối thông minh" with a one-line explanation, then each
   sub-toggle with a short Vietnamese description and its parameters (disabled/greyed when master is off), reviewer
   provider/model selects, and an editable routing table (kind → assignee kind). Save on change. Show a small badge
   "Điều phối: BẬT/TẮT" in the sidebar bottom.
3. Event log: `src/orchestration.rs` with `EventLog` appending JSON lines to `<config_dir>/ai-hub/orchestration/events.jsonl`
   (rotate at 5 MB → events.1.jsonl). Event: {ts, plan_id?, task_id?, provider_id, kind:
   "task_start|task_end|retry|watchdog_kill|resume|verify|review|fix_task|error", ok, duration_ms?, tokens?, detail}.
   Only written when enabled && event_log. Endpoint GET `/api/orchestration/events?limit=200` (newest first).
   Log task_start/task_end (with tokens parsed from the "Token:" log line, duration, ok) for plan tasks run via /api/chat
   — add optional `plan_id`/`task_id` fields to the chat request so the server can attribute events.
4. Retry (effective retry_api): in openai_compat.rs and anthropic.rs wrap the request SEND (before any streamed byte was
   forwarded) with retry on: connection errors, timeouts, HTTP 429/500/502/503/504. Backoff base_ms * 2^n (+ jitter
   0–250ms). Each retry emits a `log` "Thử lại lần n/N sau lỗi …" and an event `retry`. Never retry after streaming started.
   Respect cancellation during the backoff sleep. Unit-test the retry decision + backoff schedule (pure functions).
5. Codex watchdog (effective codex_watchdog) in cli_support/codex_cli: if a CLI child produces no stdout line for
   watchdog_minutes, kill the process tree, emit error "Codex không phản hồi sau N phút — đã dừng", event `watchdog_kill`.
6. Codex auto-resume (effective codex_auto_resume): after a codex_cli turn ends successfully, if the final agent message
   indicates it stopped early (case-insensitive contains any of: "incomplete", "not finished", "chưa xong",
   "chưa hoàn thành", "remaining work", "I stopped"), automatically run `codex exec resume <thread_id>` with prompt
   "Tiếp tục và hoàn thành phần còn lại của nhiệm vụ. Báo cáo rõ những gì chưa xong." up to resume_max times, in the
   same SSE stream (log "Tự tiếp tục lần n/N"), event `resume`. Make the trigger phrases a const list + unit test.

Update plans/orchestration.md "Nhật ký tiến độ" with one line for 10a (what was done). Reply per RULES.md.
