Implemented Step 3.

- Codex item-level errors now appear as **“Cảnh báo: …”** log events. Turn usage and Claude result usage/cost are logged, with the latest token line visible under the assistant reply and in the collapsed **Hoạt động** summary.
- Added per-chat **Quyền** settings, defaulting to **Được sửa file**. Codex receives the corresponding sandbox setting; Claude Code receives `acceptEdits` or `plan`.
- Codex no longer receives `-C`. CLI working directories are resolved before launch, and paths containing `%` or `"` produce an error event.
- Updated Vietnamese CLI support messages and the README with provider, permission, storage, and Claude Code installation details.

**Files changed:** [README.md](C:/TOOL/ai-hub/README.md), [chat.rs](C:/TOOL/ai-hub/src/chat.rs), [server.rs](C:/TOOL/ai-hub/src/server.rs), [mod.rs](C:/TOOL/ai-hub/src/providers/mod.rs), [codex_cli.rs](C:/TOOL/ai-hub/src/providers/codex_cli.rs), [claude_cli.rs](C:/TOOL/ai-hub/src/providers/claude_cli.rs), [cli_support.rs](C:/TOOL/ai-hub/src/providers/cli_support.rs), [app.js](C:/TOOL/ai-hub/ui/app.js), [index.html](C:/TOOL/ai-hub/ui/index.html), [style.css](C:/TOOL/ai-hub/ui/style.css).

**Verification:** `cargo build --release --offline` passed; `cargo clippy --offline` passed without warnings. I did not launch Codex or Claude. An additional `cargo fmt -- --check` reports formatting differences, including in untouched provider files.