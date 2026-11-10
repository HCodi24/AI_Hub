Implemented the Claude Code CLI web login flow. The UI now opens the returned link in a browser, accepts the confirmation code, polls login status, and offers a fresh login retry after failure. The backend keeps one hidden CLI session, validates its URL and submitted code, expires idle sessions, and redacts the code from returned output.

**Files changed:** [providers.rs](/C:/TOOL/ai-hub/src/api/providers.rs), [app.rs](/C:/TOOL/ai-hub/src/app.rs), [mod.rs](/C:/TOOL/ai-hub/src/api/mod.rs), [index.html](/C:/TOOL/ai-hub/ui/index.html), [main.js](/C:/TOOL/ai-hub/ui/js/main.js), [settings.js](/C:/TOOL/ai-hub/ui/js/settings.js), [settings.css](/C:/TOOL/ai-hub/ui/styles/settings.css), [README.md](/C:/TOOL/ai-hub/README.md), and [SPEC.md](/C:/TOOL/ai-hub/docs/SPEC.md).

**Verification:**

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.
- `scripts/verify.ps1` — formatting, Clippy, 46 tests, JavaScript syntax, and encoding checks passed. The script still reports a line-length failure: its log lists 96 UI lines over 200 characters across the repository.
- `scripts/smoke.ps1` — passed.
- `git diff --check` — passed. Search found no remaining `cmd /C start ... auth login` path.

**Not done:** I did not run a real Claude login or exercise the live sign-in flow, as requested. The line-length findings remain unresolved.