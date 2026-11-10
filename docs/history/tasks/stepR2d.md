Step R2d of AI Hub refactor. FIRST read docs/CODEX_RULES.md and docs/design/refactor.md. PURE MOVE, no behaviour change.
Build with CARGO_TARGET_DIR=target/codex. Gate at the end: scripts/verify.ps1 and scripts/smoke.ps1 must both pass.

SMALL SCOPE — do exactly this and nothing else:
Move ALL handlers and their private helpers for these route groups out of src/server.rs, unchanged:
  A) `/api/chat`, `/api/chat/{id}/stop`, `/api/chats*` routes (incl. the chat streaming logic, provider run orchestration, event persistence, job counter) → src/api/chat.rs
  B) `/api/config` (GET/PUT) → src/api/config.rs
Method (mechanical):
  1. For each handler: cut the function (and helpers used ONLY by it) from server.rs, paste into the target file.
  2. In the target file add the needed `use` lines (axum, serde, crate::app::AppState, crate::api::{ApiError, ApiResult},
     crate::core::..., etc.). Make the moved handlers `pub(crate)`.
  3. In server.rs's router, reference them as `crate::api::chat::<handler>` / `crate::api::config::<handler>`.
  4. Move the unit tests of moved helpers too.
  5. `cargo build` after each group; fix only import/visibility errors.
Do not touch other route groups. Do not reformat unrelated code.
Report: line counts of server.rs before/after, api/chat.rs, api/config.rs; gate results.
Update docs/design/refactor.md log.
