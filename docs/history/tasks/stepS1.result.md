Implemented S1’s chat status and hang protection.

The shared chat SSE loop sends `status` first, then `heartbeat` events every three seconds. Heartbeats track elapsed time, provider idle time, and the latest log activity; they are not saved to chat history and stop when the run ends or is cancelled. A configurable idle timeout now applies across providers, defaults to 10 minutes, accepts 0 to disable it, and is capped at 120 minutes.

The composer shows the provider, model, effort, elapsed time, and recent activity while a response runs. After 45 seconds without a provider event it shows a warning and emphasizes Dừng; after 120 seconds it also suggests stopping and resending. The streaming assistant message has a live running indicator. Stream failures or missing `done` events display the connection-loss message.

Claude CLI now handles result subtypes and missing results, emits a successful result when no deltas arrived, streams partial text without duplicating completed text, and reports relevant rate-limit events. The duplicate verbatim-path helper in `src/providers/mod.rs` was removed in favor of `crate::core::text::strip_verbatim`.

**Files changed:** `README.md`, `docs/SPEC.md`, `src/api/chat.rs`, `src/api/config.rs`, `src/core/config.rs`, `src/providers/claude_cli.rs`, `src/providers/mod.rs`, `ui/index.html`, `ui/js/chat.js`, `ui/js/main.js`, `ui/js/settings.js`, `ui/styles/chat.css`, and `ui/styles/mutis.css`.

**Verification:** `scripts/verify.ps1` passed formatting, Clippy, tests (57 passed), JavaScript syntax, encoding, and line-length checks. `CARGO_TARGET_DIR=target/codex cargo build --release --offline` and `git diff --check` also passed. No real AI calls were made.

**Not done:** I did not run `scripts/smoke.ps1`. Its cleanup force-stops the `ai-hub.exe` process it launches, which conflicts with your instruction never to stop or kill AI Hub processes. The Rust changes require rebuilding and restarting the app to take effect; I left running processes untouched.