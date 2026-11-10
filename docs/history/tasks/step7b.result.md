Implemented the Step 7b fixes.

- Rewrote and formatted [extensions.rs](/C:/TOOL/ai-hub/src/extensions.rs) with the requested Vietnamese catalog text. Extension status now resolves through the shared CLI helpers and runs the version check asynchronously with a 10-second timeout and child cleanup on timeout.
- Updated [server.rs](/C:/TOOL/ai-hub/src/server.rs) to filter a candidate inside a `.cmd` candidate’s `node_modules` directory when their versions match. Added a unit test for that rule.
- Added catalog tests for clean strings and the exact `Cài đặt` label.
- Ran `cargo fmt` across the crate; it also reformatted [claude_cli.rs](/C:/TOOL/ai-hub/src/providers/claude_cli.rs) and [cli_support.rs](/C:/TOOL/ai-hub/src/providers/cli_support.rs).
- Scanned `src/` and `ui/` for the requested mojibake markers; no hits remain.

Verification passed:

- `CARGO_TARGET_DIR=target-codex cargo build --release --offline`
- `cargo clippy --offline` — no warnings
- `cargo test --offline` — 5 passed

CodeGraph was not installed or run.