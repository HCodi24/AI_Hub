Implemented Step 9 across five files:

- [cli_support.rs](/C:/TOOL/ai-hub/src/providers/cli_support.rs): added Claude desktop app discovery and newest-version fallback, Claude environment filtering, auth JSON parsing, and unit tests.
- [server.rs](/C:/TOOL/ai-hub/src/server.rs): added Claude auth status checks, the login endpoint, and discovery of Claude app installs.
- [claude_cli.rs](/C:/TOOL/ai-hub/src/providers/claude_cli.rs): maps Claude’s “Not logged in” result to the Settings login guidance.
- [app.js](/C:/TOOL/ai-hub/ui/app.js) and [index.html](/C:/TOOL/ai-hub/ui/index.html): added the Claude login button and connection polling, and corrected Vietnamese text containing question marks.

Verification passed: release build with `CARGO_TARGET_DIR=target-codex` and `--offline`, Clippy with warnings denied, all 11 offline tests, and `node --check ui/app.js`. The UTF-8 scan found no replacement characters or specified mojibake patterns in `src/` or `ui/`, and no Vietnamese question-mark corruption matching the scan. I did not run Claude. **Nothing requested remains undone.**