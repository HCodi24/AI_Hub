Implemented R1b.

- Added [verify.ps1](/C:/TOOL/ai-hub/scripts/verify.ps1) with a UTF-8 BOM, UTF-8 console output, logged checks, and a report-only line-length warning.
- Added [smoke.ps1](/C:/TOOL/ai-hub/scripts/smoke.ps1), which builds into `target\smoke`, uses a fresh isolated config seeded by app startup, checks the routes, and stops the process it started.
- Changed the missing-config `verify_ignore` default and UI fallback to `["docs/**", "**/*.md"]`; existing user configs remain untouched.
- Put `scripts/verify.ps1` first in MutiS verify auto-detection and updated its unit test.
- Confirmed the dev watcher tracks `ui/`, `src/`, `Cargo.toml`, and `Cargo.lock`; it ignores `docs/` and `scripts/`. The self-dev test command retains `target\dev-check`.
- Updated the README and marked R1b complete in [refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md).

**Verification results**

- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1` — **ĐẠT**. fmt, clippy, tests, Node checks, and encoding scan passed. Line-length check reported 126 lines over 200 characters as warnings only.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke.ps1` — **SMOKE: ĐẠT**.
- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.

Nothing remains undone for R1b.