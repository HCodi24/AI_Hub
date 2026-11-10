Step D3 of AI Hub — two action buttons after a debate. FIRST read docs/CODEX_RULES.md and docs/SPEC.md (Tranh luận, Kế hoạch,
Bộ nhớ dự án). Build with CARGO_TARGET_DIR=target/codex and `--offline`. Gate: scripts/verify.ps1 + scripts/smoke.ps1 (smoke only
kills the PID it started). No real AI calls. Never stop ai-hub processes. UTF-8 + LF only (never PowerShell Set-Content/Out-File);
keep Vietnamese text intact. Working tree is clean (git) — you may edit any file, but keep changes minimal and inside the
debate/plan modules where possible.

USER WANTS: when a debate is finished (verdict exists), the debate card IN THE CHAT (ui/js/debate-card.js, also used by the
history page) shows two buttons instead of the current single "✦ Chia việc từ phán quyết":

A. "✦ Chia việc & thực hiện" — make a plan from the verdict, then execute it.
   1. GET /api/debates/{id} → plan_request + folder; set folderInput to debate.folder; call planFromRequest(plan_request)
      (ui/js/plans.js, already returns the plan). Store plan id on the debate (see 3).
   2. Show the plan inside the card as a compact task list (title · assignee · kind · status ○/◐/✓/✗) with a link
      "Mở kế hoạch" (openPlan). Do NOT navigate away from the chat automatically (today planFromRequest switches to the plans
      view — add an option `{ stay: true }` / separate function so the chat stays visible; keep old behaviour for other callers).
   3. Ask once with a small inline confirm in the card: "Duyệt và chạy N công việc?" [Chạy] [Để tôi xem trước]. On Chạy:
      mark plan approved (same as the "Duyệt kế hoạch" button, persistPlan) and run ALL tasks in dependency order using the
      existing per-task runner `runPlanTask(task)` (+ its gates/review/fix logic). Add an exported `runWholePlan(plan, onProgress)`
      in plans.js: sequential topological order (tasks whose depends_on are all done), skip tasks already done, stop on the
      first failed task (status ✗, show its error, button "Chạy tiếp" to resume), cancellable (button "Dừng" → abort the current
      task the same way the plan editor's stop does). Update the card list live via onProgress.
   4. If the plan view's own run-all exists already, reuse it instead of duplicating.
B. "▶ Thực hiện với…" — execute the verdict with ONE chosen AI in the chat.
   1. Click → small popover in the card: select of connected sources (state.config.providers; use the existing connection
      status/test helpers, show disconnected ones disabled with "chưa kết nối"), model select from provider.models (default the
      provider's first / last used), effort select when supported (reuse EFFORTS / effort support helpers from mode-chip.js),
      permission "Được sửa file" by default (for openai_compat agent-capable sources set file_tools "write"; for sources that
      cannot edit files show a note "Nguồn này chỉ chat, không sửa file").
   2. Confirm → switch the chat's composer controls to that provider/model/effort/permission (same functions the mode picker
      uses, so the choice is visible and remembered for the chat) and send ONE normal chat message through the existing
      sendMessage path, content = a Vietnamese instruction + the debate's plan_request: "Thực hiện phán quyết của cuộc tranh luận
      dưới đây. Làm từng bước, chạy kiểm tra nếu có, cuối cùng báo đã sửa file nào và còn gì chưa làm." + plan_request.
      It must NOT be routed to MutiS planning even if mutis.auto_plan is on, and must not start another debate even if the
      ⚖ debate switch is on (add an explicit bypass flag to sendMessage, e.g. sendMessage({ text, direct: true })).
3. Persistence: add optional `plan_id: Option<String>` to the Debate record (serde default) + PUT/PATCH-free way: a small
   endpoint POST /api/debates/{id}/plan {plan_id} that validates the plan exists and saves it. The card reads debate.plan_id on
   load (replace the current localStorage `debate-plan-<id>` hack) so reopening the chat shows the plan list with live statuses
   from GET /api/plans/{id}. Progress bar step "Chia việc" becomes done when plan_id is set; add a step "Thực hiện" (done when all
   tasks done, running ◐ while running, ✗ when one failed).
4. Buttons only appear when status == "done" and a verdict exists; disabled while the debate, the plan run or a chat stream is
   running. Both buttons work on the history page too (B there: open the debate's chat if it has one (chat_id stored? add
   optional `chat_id` to Debate when started from a chat) else start a new chat in debate.folder).
5. Tests: Rust unit tests for the new endpoint/record field (round-trip, unknown plan → 404, bad id → 400). JS: a pure helper for
   the topological run order (exported, unit-testable shape) and node --check clean.
6. Docs: README (Tranh luận: hai nút sau phán quyết) + SPEC (endpoint, fields).
Reply per docs/CODEX_RULES.md and list anything not done.
