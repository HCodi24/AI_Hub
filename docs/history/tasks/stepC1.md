Step C1 of AI Hub — everything happens IN THE CHAT: no separate page for Chia việc (plans) or Tranh luận (debates).
FIRST read docs/CODEX_RULES.md and docs/SPEC.md (Kế hoạch, Tranh luận, Bộ nhớ dự án). Build with CARGO_TARGET_DIR=target/codex and
`--offline`. Gate: scripts/verify.ps1 + scripts/smoke.ps1 (smoke only kills the PID it started). No real AI calls. Never stop
ai-hub processes. UTF-8 + LF only (never PowerShell Set-Content/Out-File); keep Vietnamese text intact. Working tree is clean.

USER (Vietnamese): "tôi không muốn kiểu chia việc phải mở trang khác như này đâu hay tranh luận cũng vậy hiện thị ngay ở khung
chat thôi". Today planFromRequest / openPlan / renderPlanEditor (ui/js/plans.js) and debate.js (openDebate, debateFromRequest,
renderDebate) hide .main-panel and show #plans-view. The debate card (ui/js/debate-card.js + debate-actions.js) is already
rendered inside the chat when started from the chat; the plan editor is only on the plans page.

GOAL
1. Plan card in the chat — new files ui/js/plan-card.js + ui/styles/plan-card.css (import from main.css), same visual language
   as the debate card (dark/light, 375px OK). One card = one plan:
   - Header "✦ Chia việc · <title>" + status (Đang lập kế hoạch… mm:ss with spinner and the live planner log lines collapsed
     under "Nhật ký", Bản nháp, Đã duyệt, Đang chạy n/N, Xong, Lỗi) + "Hủy" while planning.
   - Task list: one row per task: status icon ○/◐/✓/✗, title, assignee chip, kind, depends_on badges; click row → expands
     detail (editable textarea), assignee select (connected sources / MutiS roles exactly like the current editor offers),
     model + effort selects, permission, a "▶ Chạy task này" button, the task log/summary/verify/review output (collapsible),
     delete task, "+ Thêm task". Reuse the existing editor logic/helpers from plans.js (move pure helpers out rather than
     duplicating); keep cycle detection (hasCycle) and persistPlan.
   - Footer actions: "Duyệt kế hoạch" (when draft), "▶ Chạy tất cả" (runWholePlan, after approval; with the inline confirm used
     by the debate card), "Dừng", "Chạy tiếp" after a failure, "⇅ Xác minh kế hoạch" if the current editor has verify/plan-diff.
   - The debate card's "Chia việc & thực hiện" must use this plan card (render the plan card right after the debate card in the
     chat instead of its own mini task list) — remove the duplicated list from debate-card/debate-actions.
2. Persistence in the chat: add optional `plan_id: Option<String>` to ChatMessage (serde default, skip if none) like debate_id.
   POST /api/plans accepts optional `chat_id`; when valid, the server appends the user request message (if the request came
   from the chat composer) and an assistant message with plan_id (content "" while planning, then a short markdown summary of
   the plan: title + task list with assignees, ≤ ~1500 chars, updated when tasks finish so later chat turns / the memory feature
   see the progress). renderMessages() renders a message with plan_id as the plan card loaded from GET /api/plans/{id}
   (cached; live card reused). Reopening the chat shows the same card with current task statuses.
   Messages with plan_id/debate_id must not be resent as normal provider history except through their summary content.
3. Entry points all stay in the chat:
   - "✦ Gợi ý chia việc" button, the big-request hint, MutiS auto_plan, and the dev "Chia việc cho app" button → append the plan
     card to the CURRENT chat (create the chat if needed) and stream planning into it; never switch view.
   - Debates started anywhere → debate card in the current chat.
   - Running a task from the card streams its output inside that task row (keep using the existing per-task chat runs; the
     task's own chat id stays as today) — no navigation.
4. Remove the separate page: delete #plans-view markup and the sidebar "✦ Chia việc" page button; the sidebar instead shows
   nothing extra (plans/debates live in their chats) — but keep a small "Kế hoạch & tranh luận cũ" list ONLY for records that
   have no chat (legacy): clicking one creates a new chat titled after it in its folder and appends the card. Delete dead code
   (renderPlanList page rendering, debate.js page rendering) — keep API functions other modules import (planFromRequest,
   runWholePlan, runPlanTask, persistPlan, loadPlans, debateFromRequest…) with the same names, adapted to render into the chat.
   Planner provider/model selects that lived on the plans page (#planner-provider, #planner-model) must still exist somewhere
   the code reads them (move them into Settings › Chia việc, or derive from MutiS lead) — grep every use and keep behaviour.
5. Tests: Rust unit tests for chat_id on POST /api/plans (append + summary update, invalid chat id handling, plan_id round-trip).
   JS: node --check, existing plan-order / plan-run tests must still pass; add a small test for the plan summary text helper.
6. Docs: README + SPEC (no plans page; cards in chat; chat_id/plan_id).
Reply per docs/CODEX_RULES.md and list anything not done.
