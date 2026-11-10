Completed R2e as a pure move. Provider management, model loading, connection tests and cache, CLI status, and CLI discovery handlers and helpers are now in [src/api/providers.rs](/C:/TOOL/ai-hub/src/api/providers.rs). The router and the shared cache consumers now reference that module. No behavior change was intended.

**Line counts:** `src/server.rs` went from 1,520 to 616 lines; `src/api/providers.rs` is 936 lines. I did not create `src/providers/discovery.rs`; the discovery helpers moved with their HTTP handler.

**Verification passed:**

- `CARGO_TARGET_DIR=target/codex cargo build`
- `scripts/verify.ps1` — fmt, clippy, tests, Node syntax, and encoding checks passed
- `scripts/smoke.ps1`

PowerShell required a process-scoped `ExecutionPolicy Bypass` to run the gate scripts. The R2e result is recorded in [docs/design/refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md).

No group B work was done. The pre-existing untracked `docs/history/tasks/stepR2e.md` was left untouched.