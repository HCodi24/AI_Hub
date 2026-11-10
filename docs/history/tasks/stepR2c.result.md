**R2c is complete.** The specified handlers and private helpers are in the three API modules, and `src/server.rs` routes to them. The extension helper tests moved with the helper. Updated the log in [refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md).

| File | Lines |
|---|---:|
| `src/server.rs` before | 2,653 |
| `src/server.rs` after | 1,948 |
| `src/api/mutis.rs` | 520 |
| `src/api/verify.rs` | 86 |
| `src/api/extensions.rs` | 190 |

**Verification passed:** `scripts/verify.ps1` (including fmt, clippy, tests, Node syntax, and encoding checks), `scripts/smoke.ps1`, and `CARGO_TARGET_DIR=target/codex cargo build --release --offline`. The line-length check in `verify.ps1` reported existing long lines as informational.

The first compile after group A used Cargo’s default target directory; subsequent builds, including the final release build, used `target/codex`. I left the untracked `docs/history/tasks/stepR2c.md` untouched.