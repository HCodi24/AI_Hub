Implemented the Step 1 review fixes and Step 2 CLI providers.

### Files changed

- [server.rs](/C:/TOOL/ai-hub/src/server.rs) removes the moved-sender drop and unused import, throttles chat-file writes, persists session IDs, and rejects disallowed Host and Origin headers.
- [cli_support.rs](/C:/TOOL/ai-hub/src/providers/cli_support.rs) adds executable lookup, safe `.cmd` launching, stdin prompt delivery, stderr capture, cancellation, and process-tree termination.
- [codex_cli.rs](/C:/TOOL/ai-hub/src/providers/codex_cli.rs) implements Codex execution, resume, event parsing, and model/session validation.
- [claude_cli.rs](/C:/TOOL/ai-hub/src/providers/claude_cli.rs) implements Claude stream-json parsing, resume, tool logs, and the friendly missing-CLI error.
- [mod.rs](/C:/TOOL/ai-hub/src/providers/mod.rs) wires both CLI providers into the Provider abstraction and adds shared error helpers.
- [openai_compat.rs](/C:/TOOL/ai-hub/src/providers/openai_compat.rs) and [anthropic.rs](/C:/TOOL/ai-hub/src/providers/anthropic.rs) include sanitized response bodies on HTTP errors and surface SSE error objects.
- [main.rs](/C:/TOOL/ai-hub/src/main.rs) passes the actual port to the security middleware.
- [config.rs](/C:/TOOL/ai-hub/src/config.rs) adds an optional `AI_HUB_CONFIG_PATH` override for isolated runs; the normal config location is unchanged when it is unset.

The existing UI already renders live “Hoạt động” entries and CLI status, so I left its files unchanged.

### Verification

- `cargo build --release --offline` completed successfully.
- `cargo clippy --offline` completed without warnings.
- Started the release server on port `7899`; `/api/cli-status` reported Codex CLI `0.167.0` and Claude CLI unavailable.
- Requests with an unapproved Host and Origin both returned `403`.
- Posted the requested harmless Vietnamese prompt to `codex-cli` from a temp folder. The SSE response contained `error` and `done`, but no `delta`: Codex exited before inference with `Could not find home directory`. A direct probe with an explicit Codex home also hit sandbox access denial while initializing under the user profile. The server was stopped afterward.

### Codex JSONL observation and unfinished verification

I checked `codex exec resume --help`; this installed version supports `resume [OPTIONS] [SESSION_ID] [PROMPT]`, so the implementation places options before the session ID. **No Codex JSONL event types were observed:** the CLI failed before emitting JSONL. The parser handles the documented thread, turn, item, message, reasoning, command, file-change, tool, and error shapes, but their live mapping remains unverified here. Claude execution also remains unverified because its CLI is not installed.

The optional cleanup command for the smoke-test directories was blocked by shell policy, so these harmless artifacts remain under `%TEMP%`: `ai-hub-step2-runtime`, `AI Hub CLI Smoke`, and `ai-hub-codex-step2-probe`. `.tasks/step2.log` was already untracked when I started and was left untouched.