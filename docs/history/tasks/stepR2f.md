Step R2f of AI Hub refactor. FIRST read docs/CODEX_RULES.md and docs/design/refactor.md. PURE MOVE, no behaviour change.
Build with CARGO_TARGET_DIR=target/codex. Gate at the end: scripts/verify.ps1 and scripts/smoke.ps1 must both pass.

Do exactly this:
1. Move every `/api/plans*` handler and plan-generation helpers (planner prompt, plan SSE, reassignment glue, plan events)
   from src/server.rs → src/api/plans.rs (pure plan logic that does not touch HTTP may go to src/plans.rs).
2. Move the router assembly (`router()` with all `.route(...)` lines) and the Host/Origin middleware from src/server.rs
   into src/api/mod.rs as `pub fn router(state: AppState) -> Router`. Group routes by feature with a comment per group.
3. Move anything still left in src/server.rs to the module it belongs to (api/*, core/*, providers/*), then DELETE
   src/server.rs and remove `mod server`. Fix all `crate::server::...` paths (e.g. strip_verbatim now lives in core::text).
4. Update docs/design/refactor.md log and the "Cấu trúc mã nguồn" table in README.md to the new files.
Report final line counts of src/**/*.rs and gate results.
