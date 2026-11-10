## R2d complete

Moved the requested chat and config handlers into `src/api/chat.rs` and `src/api/config.rs`, updated the router references, and added the R2d entry to [refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md). No behavior changes were intended. No unit tests for the moved chat helpers were present in `server.rs`.

Line counts:

- `src/server.rs`: **1,999 → 1,556**
- `src/api/chat.rs`: **439**
- `src/api/config.rs`: **45**

Verification passed:

- `CARGO_TARGET_DIR=target/codex cargo build --offline`
- `scripts/verify.ps1` — passed (run with PowerShell execution-policy bypass)
- `scripts/smoke.ps1` — passed (run with PowerShell execution-policy bypass)
- `git diff --check` — passed

Nothing requested remains undone. I left the pre-existing untracked `docs/history/tasks/stepR2d.md` untouched.