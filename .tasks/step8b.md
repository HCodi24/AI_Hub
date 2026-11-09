Step 8b — fix review findings for "Chia việc" in AI Hub (C:\TOOL\ai-hub). Build with
`CARGO_TARGET_DIR=target-codex cargo build --release --offline`, clippy, `cargo test --offline`; all clean.
Also run `node --check ui/app.js`. No real AI calls.

Reviewer live test: POST /api/plans with planner codex-cli took 3 minutes and produced a sensible 4-task plan, but
assigned tasks 1 and 4 to `claude-cli`, which is NOT installed on this machine.

1. READABILITY: the step-8 code in ui/app.js (renderPlannerSettings, savePlannerConfig, loadPlans, renderPlanList,
   planFromRequest, openPlan, renderPlanEditor, persistPlan, runPlanTask, and the new listeners) and the plans-view
   markup in ui/index.html are minified one-liners. Rewrite them as normal readable, multi-line code with small
   helper functions (e.g. el(tag, props, children)). Same for any other minified code added in steps 6–8
   (runExtensionAction etc.). Keep behaviour.
2. Planning feedback: generation can take minutes. While planning: disable both "Gợi ý chia việc" buttons, open the
   Chia việc view immediately with a card "Đang lập kế hoạch bằng <provider> · <elapsed mm:ss>" + spinner + a
   "Huỷ" button (AbortController; backend must kill the planner process when the request is dropped — verify the
   planner call uses the same cancellation as chat). Better: make POST /api/plans stream SSE `log` events (CLI
   activity) then a final `plan` event, and show those logs in the card.
3. Only offer CONNECTED workers to the planner: backend runs the provider tests (in parallel, cached ~60s) and
   lists only connected providers as available assignees in the prompt. After parsing, any task whose assignee is
   unknown or not connected is reassigned to the best connected provider for its kind (code/test -> codex_cli if
   connected, docs/test -> an openai_compat provider if connected, plan/review -> planner) and flagged
   `reassigned_from` so the UI shows "⚠ đã đổi từ claude-cli (chưa kết nối)".
4. Planner default: first CONNECTED of claude_cli, anthropic, codex_cli (spec), not merely first existing.
5. runPlanTask bug: after an `error` event the status is overwritten with "xong" at the end. Final status must be:
   stopped -> "đã dừng", any error -> "lỗi", else "xong". Reset `t.summary` at the start of a (re)run.
6. Per-task model/effort: add optional `model` and `effort` to PlanTask (default: assignee's first model, effort
   "default"); show them in the row (model select from the provider's models) and use them when running.
7. Do not re-render the whole plan editor on every streamed event (it steals focus from inputs and is slow):
   update only that task's status/log/token elements; full re-render only on structural changes.
8. The dependency input: validate ids exist and reject cycles (show inline error, block "Duyệt kế hoạch").
Add unit tests (Rust) for: JSON extraction from a planner answer with surrounding text, reassignment rule, cycle
detection (implement cycle check server-side on PUT /api/plans/{id} when approved=true -> 400).
Reply with files changed and verification.
