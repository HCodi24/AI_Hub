## Implemented I1

Added image attachments to the chat composer. Users can paste, drag and drop, or choose PNG, JPEG, WebP, and GIF images. The composer enforces five images per message and 10 MB per image, shows removable thumbnails, and allows image only messages. Sent images appear in chat history and open at full size in a new tab.

Added `POST /api/attachments` and `GET /api/attachments/{chat}/{file}`. Uploads are checked by magic bytes and stored under the app data directory, separated by chat UUID. Chat records persist attachment IDs, MIME types, and server resolved paths. Provider handling follows the requested rules for Claude CLI, Codex CLI, Anthropic, OpenAI compatible, DeepSeek agent, and MutiS normal chat. Added tests for image type validation, serving path safety, Codex image path fencing and arguments, and Anthropic/OpenAI message builders.

Updated [README.md](/C:/TOOL/ai-hub/README.md), [SPEC.md](/C:/TOOL/ai-hub/docs/SPEC.md), and the smoke script’s request timeout. The timeout now allows for the existing local CLI probes used by MutiS role resolution.

## Files changed

- Backend and providers: [attachments.rs](/C:/TOOL/ai-hub/src/api/attachments.rs), [chat.rs](/C:/TOOL/ai-hub/src/api/chat.rs), [mod.rs](/C:/TOOL/ai-hub/src/api/mod.rs), [chat.rs](/C:/TOOL/ai-hub/src/features/chat.rs), [anthropic.rs](/C:/TOOL/ai-hub/src/providers/anthropic.rs), [claude_cli.rs](/C:/TOOL/ai-hub/src/providers/claude_cli.rs), [codex_cli.rs](/C:/TOOL/ai-hub/src/providers/codex_cli.rs), [deepseek_agent.rs](/C:/TOOL/ai-hub/src/providers/deepseek_agent.rs), [mod.rs](/C:/TOOL/ai-hub/src/providers/mod.rs), [openai_compat.rs](/C:/TOOL/ai-hub/src/providers/openai_compat.rs), [mutis.rs](/C:/TOOL/ai-hub/src/api/mutis.rs), and [plans.rs](/C:/TOOL/ai-hub/src/api/plans.rs).
- UI: [index.html](/C:/TOOL/ai-hub/ui/index.html), [chat.js](/C:/TOOL/ai-hub/ui/js/chat.js), [main.js](/C:/TOOL/ai-hub/ui/js/main.js), and [chat.css](/C:/TOOL/ai-hub/ui/styles/chat.css).
- Docs and gate: [README.md](/C:/TOOL/ai-hub/README.md), [SPEC.md](/C:/TOOL/ai-hub/docs/SPEC.md), and [smoke.ps1](/C:/TOOL/ai-hub/scripts/smoke.ps1).

## Verification

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.
- `scripts/verify.ps1` — passed: formatting, Clippy, 52 tests, JavaScript checks, encoding scan, and line length.
- `scripts/smoke.ps1` — passed.
- `git diff --check` — passed.

**Not done:** Interactive browser testing was not performed. No real AI calls were made, as requested.