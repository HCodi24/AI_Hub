**R2b is complete.** The requested routes and their helpers were moved without intended behavior changes. The `dev_chat_folder` unit test moved with its helper.

| File | Lines |
|---|---:|
| `src/server.rs` before | 3156 |
| `src/server.rs` after | 2653 |
| `src/api/dev.rs` | 491 |
| `src/api/static_ui.rs` | 52 |

The changes are in [server.rs](/C:/TOOL/ai-hub/src/server.rs), [dev.rs](/C:/TOOL/ai-hub/src/api/dev.rs), [static_ui.rs](/C:/TOOL/ai-hub/src/api/static_ui.rs), [app.rs](/C:/TOOL/ai-hub/src/app.rs), and [refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md). The `app.rs` update points startup at the moved dev watcher.

**Verification:** `CARGO_TARGET_DIR=target/codex cargo build --offline`, `scripts/verify.ps1`, and `scripts/smoke.ps1` passed. Verify reported fmt, clippy, test, node, and encoding checks passing; line length was report-only. `git diff --check` also passed.

**Not done:** Nothing requested for R2b remains.