**R2a is partial, not complete.** The two required gates passed, and the release build succeeded, but I did not finish moving the requested HTTP handlers into `src/api/`. The new API feature files are placeholders, and `api::router` still delegates to `server::router`.

Completed moves include `config.rs` to `core/config.rs` with `crate::config` compatibility, `AppState` to `app.rs`, shared text helpers to `core/text.rs`, path helpers to `core/paths.rs`, and `ApiError`/`ApiResult` to `api/mod.rs`. I updated existing callers and the refactor log. `server.rs` is **3,244 lines**.

**New files and line counts**

- `src/app.rs` — 76
- `src/core/mod.rs` — 4
- `src/core/config.rs` — 565
- `src/core/paths.rs` — 29
- `src/core/process.rs` — 47
- `src/core/text.rs` — 73
- `src/api/mod.rs` — 40
- `src/api/dev.rs` — 1
- `src/api/mutis.rs` — 1
- `src/api/verify.rs` — 1
- `src/api/extensions.rs` — 1
- `src/api/static_ui.rs` — 1

**Verification**

- `CARGO_TARGET_DIR=target/codex powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1` — passed fmt, Clippy, tests, Node, and encoding checks. The script reported overlength lines.
- `CARGO_TARGET_DIR=target/codex powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke.ps1` — passed.
- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.

The requested 200-character limit is **not met**: `server.rs`, moved `core/config.rs`, and `providers/cli_support.rs` still contain overlength lines. The handler extraction and complete shared process-helper migration also remain unfinished.