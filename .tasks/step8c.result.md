Completed A–C in AI Hub. No AI generation calls were made.

- **Connected workers and reassignment:** Added a 60-second provider status cache and parallel provider checks for planning. The planner prompt lists only connected providers with their ID, name, kind, and models. The backend selects a connected default planner when none is configured, reassigns unavailable or unknown assignees by task kind, and records the original assignee. Added unit coverage for `reassign`; the UI shows the reassignment notice.
- **Planning stream and cancel:** `/api/plans` now streams provider log events followed by a saved `plan` event, or an `error` event. Closing or aborting the request cancels the planner through the existing cancellation pattern. The UI reads SSE, displays logs, opens the completed plan, and shows “Đã huỷ” after cancellation.
- **Readability:** Reworked the listed `ui/app.js` lines over 300 characters into multiline code; no lines over 300 characters remain.

**Files changed:** [server.rs](/C:/TOOL/ai-hub/src/server.rs), [plans.rs](/C:/TOOL/ai-hub/src/plans.rs), [app.js](/C:/TOOL/ai-hub/ui/app.js).

**Verification passed:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (8 passed), and `node --check ui/app.js`.