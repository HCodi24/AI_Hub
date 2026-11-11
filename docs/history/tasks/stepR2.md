Step R2 of AI Hub — React UI: debate, plan and memory cards inside /app chat. FIRST read docs/CODEX_RULES.md,
docs/FE_STRUCTURE.md (follow it strictly — same structure as C:\GitHub\BEANS\SCM-BEANS_Dashboard_FE; never read .env*/.npmrc/
secrets there) and docs/SPEC.md (Tranh luận, Kế hoạch, Bộ nhớ dự án, Quyền). Repo C:\GitHub\AI_Hub. Rust: CARGO_TARGET_DIR=
target/codex `--offline`. No network: use only packages already in web/node_modules. No real AI calls. Never stop ai-hub
processes and do NOT leave servers running when you finish (stop only processes you started). UTF-8 + LF only (never
PowerShell Set-Content/Out-File); Vietnamese intact. Do not touch _restructure_ui.py. The legacy UI (ui/) must keep working.

Port to React (web/), behaviour identical to the legacy cards (read them: ui/js/debate-card.js, debate-actions.js, debate.js,
plan-card.js, plan-helpers.js, plan-summary.js, plan-order.js, plans.js (runPlanTask/runWholePlan/gates/review/fix),
memory.js, chat.js (debateInChat, auto_plan, switches), mode-chip.js) and to the Rust APIs (src/api/{debate,plans,memory,
verify,mutis,chat}.rs). Replace the R1 placeholder cards.

1. apis + schema: debates.api.ts/debates.schema.ts (list, get, start SSE with chat_id, plan link), plans.api.ts/
   plans.schema.ts (list, get, save PUT, generate SSE with chat_id, verify, plan-diff), memory.api.ts/memory.schema.ts
   (overview, get/put scope, archive/restore/delete), mutis.api.ts/mutis.schema.ts (config get/put incl. roles, routing,
   switches like auto_plan/verify_gate/auto_review/auto_fix, roles resolve). Zod everywhere, no `any`.
2. features/debate/: debate-card (header + timer, progress steps incl. Chia việc/Thực hiện, verdict first with chips,
   round tabs, two columns Codex|Claude → one column + toggle < 700px, collapsed bodies rendered with the shared
   markdown-view, stance badges, tokens formatted vi-VN, >500k warning, live activity line, Stop), debate-actions (✦ Chia việc &
   thực hiện → renders the plan card in the chat + inline confirm "Duyệt và chạy N công việc?"; ▶ Thực hiện với… popover with
   connected sources/model/effort/permission incl. Toàn quyền, sends a DIRECT chat message bypassing MutiS/auto-plan/debate).
   Starting a debate from /app: composer gets a "⚖ Tranh luận" toggle (persisted like legacy localStorage debateMode) and the
   message becomes a debate card in the current chat (POST /api/debates with chat_id).
3. features/plan/: plan-card (planning state with timer/log/Hủy; task rows with status ○◐✓✗, assignee chip, kind, depends_on;
   expandable editor: detail, assignee select (connected sources + MutiS roles as legacy), model, effort, permission incl.
   full, run this task, mark done, review again, move up/down, delete, add task; footer: Duyệt, ▶ Chạy tất cả (+ inline
   confirm), Dừng, Chạy tiếp, ⇅ Xác minh kế hoạch, Lập lại kế hoạch; task output/verify/review collapsible). Port the run
   engine (runPlanTask + gates + auto review + auto fix + runWholePlan + topological order + cancel) into
   features/plan/plan-runner.ts (pure-ish, testable) using the zustand store; reuse the same backend calls as legacy
   (POST /api/chat with plan_id/task_id/task_kind…). Entry points in /app: composer "✦ Chia việc" button, MutiS auto_plan
   when the MutiS source is selected (the MutiS source itself must exist in the source select as in legacy: "✦ MutiS (3 AI)"
   using roles.lead for normal chat), big-request hint optional.
4. features/memory/: "Bộ nhớ" button in the top bar → side panel/dialog for the chat folder (active + archived scopes, editor
   with Lưu, Lưu trữ/Khôi phục/Xoá with confirm); SSE `memory` event → toast with "Hoàn tác" for archived.
   Add a shared toast component (components/toast) + dialog (components/dialog) if missing.
5. chat-message renders messages with debate_id / plan_id as the corresponding card (load by id with React Query; live card
   state shared via the store so a running card is not reloaded).
6. Tests: extend scripts/test-web.cjs (or add web unit tests runnable with node, no new packages) for plan order, runner
   state transitions (mock fetch), plan summary, debate SSE reducer, memory toast logic. Keep legacy tests passing.
7. Quality: components < ~250 lines, strings in src/text/<area>.ts, accessible, dark+light, 375px no horizontal scroll.
   Take screenshots of the debate card, plan card and memory panel at 1100px dark and 375px light with mock data (Playwright is
   NOT installed — if you cannot screenshot, say so; do not install anything).
8. Docs: README/SPEC/FE_STRUCTURE updates. Verify: web typecheck/lint/build + scripts/verify.ps1 + scripts/smoke.ps1.
Reply per docs/CODEX_RULES.md (files, what works, verify output, anything not done).
