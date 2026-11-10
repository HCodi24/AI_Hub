Step R2a of AI Hub refactor (backend, part 1). FIRST read docs/CODEX_RULES.md and docs/design/refactor.md.
PURE REFACTOR: move code, do not change behaviour, routes, JSON shapes, messages or tests' meaning.
Build with CARGO_TARGET_DIR=target/codex. Gate (must pass at the end, run them yourself):
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1` and `... -File scripts/smoke.ps1`.

1. Create `src/core/` with `mod.rs` and:
   - `text.rs`: ONE `strip_ansi` (replace the copies in server.rs and verify.rs), `extract_json`, `strip_verbatim`,
     `one_line`/`truncate_chars` if duplicated elsewhere — move, then update all call sites.
   - `process.rs`: shared child-process helpers now in server.rs (kill tree / taskkill, CREATE_NO_WINDOW command setup,
     run-with-timeout, git_output/git_text) — move from server.rs; cli_support keeps its CLI-specific parts but uses these.
   - `paths.rs`: config/data dir helpers (chat dir, plans dir, mutis dir, backups dir) if they live in several files.
   Move `config.rs` to `src/core/config.rs` (keep `crate::config` working via `pub use` in main.rs or update paths).
2. Create `src/api/` with `mod.rs` (router assembly, Host/Origin middleware, `ApiError`/`ApiResult`, SSE helpers) and move
   the handlers of these features out of server.rs into their own files, unchanged:
   - `api/dev.rs` (all /api/dev/* handlers), `api/mutis.rs` (/api/mutis*, /api/orchestration* aliases, diagnose, apply,
     stats, events), `api/verify.rs` (/api/verify, plan-diff), `api/extensions.rs` (/api/extensions*),
     `api/static_ui.rs` (/, /app.js, /style.css incl. dev disk mode).
   Leave chat/providers/plans/config handlers in server.rs for R2b, but server.rs must import the moved pieces.
3. AppState goes to `src/app.rs` (struct + new()); main.rs uses `api::router(state)`.
4. No line in the files you touch may exceed 200 chars (rustfmt + manual wrapping of long string literals/json! calls).
5. Keep every existing unit test (move tests with their code). Add none unless needed for moved helpers.
Report: new file list with line counts, server.rs line count after, gate results. Update docs/design/refactor.md log.
