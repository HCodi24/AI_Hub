Step 8c — finish the remaining step-8b items in AI Hub (C:\TOOL\ai-hub). Same build/clippy/test/`node --check` rules.
No real AI calls. Do ONLY these three things, completely:

A. Connected workers + reassignment (backend, src/server.rs + src/plans.rs):
   - Reuse the existing provider test function (the one behind POST /api/providers/{id}/test). Add a small cache
     (HashMap<provider_id, (Instant, status)>, TTL 60s) in AppState.
   - In generate_plan: test all providers in parallel (futures::join_all), and list ONLY connected providers as
     assignees in the planner prompt (id, name, kind, models).
   - After parsing the plan: for each task whose assignee is unknown or not connected, pick a replacement with a
     pure function `reassign(kind, connected: &[ProviderSummary], planner_id) -> String`:
     code/test -> first connected codex_cli, else first connected claude_cli, else planner;
     docs -> first connected openai_compat, else codex_cli, else planner; plan/review -> planner.
     Set `reassigned_from = Some(old)`. Unit-test `reassign`.
   - Planner default when config has none: first CONNECTED of claude_cli, anthropic, codex_cli.
   - UI: show "⚠ đã đổi từ <old> (chưa kết nối)" under the assignee select when reassigned_from is set.
B. Streaming + cancel for planning:
   - POST /api/plans returns SSE: `log` events forwarded from the planner provider (CLI activity, token line),
     then one `plan` event with the saved plan JSON, or `error`. Use the same CancellationToken + CancelOnDrop
     pattern as chat so closing the request kills the planner process tree.
   - ui/app.js: planFromRequest reads the SSE (reuse readSse), appends logs to the progress card, opens the plan on
     the `plan` event; "Huỷ" aborts the fetch (AbortController) and shows "Đã huỷ".
C. Readability: these lines in ui/app.js are still > 300 chars (see .tasks/longlines.txt). Reformat each into
   readable multi-line code without changing behaviour.
Reply with what is done, files changed, and verification. If anything is not done, say so explicitly.
