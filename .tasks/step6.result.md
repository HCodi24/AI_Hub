Implemented Step 6.

- Added provider connection tests for OpenAI compatible, Anthropic, Codex CLI, and Claude Code CLI. Results include status and detail text, and never return API keys.
- Added status dots and tooltips to the Settings provider pills, a status line and **Kiểm tra kết nối** button in the edit panel, and a matching dot on the mode chip. Opening Settings starts checks for all providers in parallel; saving a provider checks it again. Results and transition toasts are kept in UI memory for the session.
- Added **Tự tìm trên máy** for both CLI sources, including candidate selection, automatic save and check for a single match, and copyable install hints when no match is found.
- Added bounded CLI discovery and version validation. Centralized executable resolution also blocks paths under `WindowsApps` and Claude Desktop’s `Claude.exe` when its directory contains `resources\app.asar`.

Files changed: [server.rs](C:/TOOL/ai-hub/src/server.rs), [cli_support.rs](C:/TOOL/ai-hub/src/providers/cli_support.rs), [app.js](C:/TOOL/ai-hub/ui/app.js), [index.html](C:/TOOL/ai-hub/ui/index.html), [style.css](C:/TOOL/ai-hub/ui/style.css).

**Verification:** `CARGO_TARGET_DIR=target-codex cargo build --release --offline` succeeded; `CARGO_TARGET_DIR=target-codex cargo clippy --offline` completed without warnings; `git diff --check` passed. I did not run Codex or Claude chats.