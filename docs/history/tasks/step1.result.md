Implemented the step 1 project and UI.

**Files created**

- Project setup: [Cargo.toml](/C:/TOOL/ai-hub/Cargo.toml), [.gitignore](/C:/TOOL/ai-hub/.gitignore), [README.md](/C:/TOOL/ai-hub/README.md)
- Rust server and providers: [main.rs](/C:/TOOL/ai-hub/src/main.rs), [config.rs](/C:/TOOL/ai-hub/src/config.rs), [server.rs](/C:/TOOL/ai-hub/src/server.rs), [chat.rs](/C:/TOOL/ai-hub/src/chat.rs), [providers/mod.rs](/C:/TOOL/ai-hub/src/providers/mod.rs), [openai_compat.rs](/C:/TOOL/ai-hub/src/providers/openai_compat.rs), [anthropic.rs](/C:/TOOL/ai-hub/src/providers/anthropic.rs)
- Embedded UI: [index.html](/C:/TOOL/ai-hub/ui/index.html), [app.js](/C:/TOOL/ai-hub/ui/app.js), [style.css](/C:/TOOL/ai-hub/ui/style.css)

The implementation includes masked API keys, provider CRUD and model loading, cancellable SSE chat, saved chat history, CLI version detection, and the Vietnamese settings and chat UI. No real API keys are hard-coded.

**Verification**

- `cargo fmt --all -- --check`, `cargo metadata --no-deps --format-version 1`, and `node --check ui/app.js` completed successfully.
- `cargo build --release` and `cargo clippy` could not reach compilation or linting: Cargo failed fetching the crates.io index with a Windows Schannel `SEC_E_NO_CREDENTIALS` error. Offline resolution also failed because `anyhow` is not cached. As a result, I can’t confirm the requested build or warning-free Clippy run, and Cargo did not generate a lockfile.

Codex CLI and Claude Code chat remain intentionally unsupported in step 1; they return **“Chưa hỗ trợ ở bước 1”** as specified.