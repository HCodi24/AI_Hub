**Step 11b is implemented.** The dev-only “Tự phát triển” page now shows repo status, refreshes it every 10 seconds, and can open a write-enabled chat or the plan flow on the repo. Git actions use fixed backend commands; reset confirms the affected files and only cleans `src` and `ui`. Test and release builds stream logs, and their results are shown in the page.

Updated [src/server.rs](/C:/TOOL/ai-hub/src/server.rs), [src/dev.rs](/C:/TOOL/ai-hub/src/dev.rs), [ui/index.html](/C:/TOOL/ai-hub/ui/index.html), [ui/app.js](/C:/TOOL/ai-hub/ui/app.js), [ui/style.css](/C:/TOOL/ai-hub/ui/style.css), [README.md](/C:/TOOL/ai-hub/README.md), and [plans/self-dev.md](/C:/TOOL/ai-hub/plans/self-dev.md).

**Verification passed:**

- `CARGO_TARGET_DIR=target-codex cargo build --release --offline`
- `cargo clippy --offline --all-targets -- -D warnings`
- `cargo test --offline` — 41 passed
- `node --check ui/app.js`
- UTF-8/mojibake scan
- `git -c core.whitespace=cr-at-eol diff --check` — clean with the repo’s CRLF line endings

The default `git diff --check` flags the CR characters on newly edited README lines; the CRLF-aware check is clean.

**Not done:** I did not exercise the page in a running app, click its test or release-build buttons, or make live AI calls. Git status also shows an untracked `.tasks/step11b.md`; I did not edit it.