Step 10d3 — MutiS self-diagnosis ONLY (the core of "Claude tự đánh giá MutiS"). FIRST read .tasks/RULES.md and
.tasks/step10d.md item 2 (the spec). Already present from 10d2: a placeholder diagnose endpoint, diagnosis parsing
helper, diagnosis history listing, a limited setting-change endpoint, the MutiS page. Keep the scope small and finish it.

1. POST /api/mutis/diagnose → SSE. If fewer than 5 events: final `diagnosis` {health:"chưa đủ dữ liệu", findings:[],
   summary:"Chưa đủ dữ liệu để chẩn đoán"} without AI. Else resolve the lead role via the same logic as
   /api/mutis/roles/resolve (connected provider; CLIs read-only), send the prompt (current MutiS settings without keys,
   stats, last 150 failure-related events truncated to 300 chars each, the JSON contract from step10d.md), forward the
   provider's `log` events, parse the answer with the existing extract_json, validate shape (unknown keys dropped), save
   to <data>/mutis/diagnoses/<ts>.json, emit `diagnosis`, and write an event kind `diagnosis`. Reuse the chat provider
   runner (do not duplicate provider code). If no lead is connected: error "Không có nguồn đang kết nối để chẩn đoán".
2. Whitelist for setting_change (server-side, single function, unit-tested): allowed paths
   mutis.{retry_api,retry_attempts,retry_base_ms,codex_watchdog,watchdog_minutes,codex_auto_resume,resume_max,
   deepseek_agent,deepseek_max_file_lines,deepseek_max_steps,verify_gate,auto_review,auto_fix,fix_rounds_max,event_log},
   mutis.roles.<lead|big|small>.<provider_id|model|effort> (provider_id must exist, effort valid),
   mutis.routing[<i>].role (lead|big|small). Everything else rejected (esp. anything containing api_key, providers, paths,
   verify_command, worker_rules). Values go through the same range validation as PUT /api/mutis.
   POST /api/mutis/apply {path, value} → returns {old, new} and saves.
3. UI (MutiS page): button "Tự chẩn đoán" (spinner + live log), result header with health badge + summary, finding cards
   (severity colour, problem, evidence, cause, suggestion). Cards whose setting_change passes the whitelist get "Áp dụng"
   → confirm dialog "Đổi <path>: <old> → <new>?" → POST /api/mutis/apply → toast. Rejected ones show "Không áp dụng tự
   động được". List of previous diagnoses (date, health) that can be opened.
4. Unit tests: whitelist accept/reject (cases listed in step10d2.md item 4), diagnosis shape validation.
Update plans/mutis.md progress log and README/SPEC to match reality. Reply per RULES.md listing anything not done.
