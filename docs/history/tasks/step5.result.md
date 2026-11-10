Implemented the mode bar and provider-specific effort handling.

Files changed:

- `ui/index.html`, `ui/app.js`, `ui/style.css` — added the chip and popover, model name formatting, effort choices, keyboard controls, and localStorage remembering. The top-bar controls stay in sync, and the hint moves beside the send actions and hides below 640px.
- `src/chat.rs`, `src/server.rs`, `src/providers/mod.rs`, `src/providers/codex_cli.rs`, `src/providers/openai_compat.rs`, `src/providers/anthropic.rs` — added effort validation and chat persistence, then applied effort per provider. Unsupported OpenAI-compatible reasoning effort retries once without the field and logs the fallback.
- `README.md` — added the “Thanh chế độ” section.

Verification passed:

- `CARGO_TARGET_DIR=target-codex cargo build --release --offline`
- `CARGO_TARGET_DIR=target-codex cargo clippy --offline`

I did not run Codex or Claude.