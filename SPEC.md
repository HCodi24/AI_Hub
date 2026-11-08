# AI Hub — spec

Single Rust binary. Local web UI to chat with / delegate coding work to several AI sources
from one place. Windows 11 is the primary target (must also compile on Linux/macOS).

## Stack
- Rust 2021, `tokio`, `axum` (0.7 or 0.8), `reqwest` (rustls, stream feature), `serde`/`serde_json`,
  `futures`, `tokio-stream`, `anyhow`, `dirs`, `uuid`, `open` (to open the browser).
- Static UI (`ui/index.html`, `ui/app.js`, `ui/style.css`) embedded into the binary with `include_str!`.
  Plain HTML/CSS/vanilla JS, no build step, no CDN.
- Server binds **127.0.0.1 only**, default port 7878 (`--port` / `AI_HUB_PORT` override).
  On start: print the URL and open it in the default browser (`--no-open` to skip).

## Sources ("providers")
Every chat request names a provider + model + working folder. Responses stream to the UI with
Server-Sent Events (SSE): events `delta` (text chunk), `log` (tool/command activity from CLIs),
`error`, `done`. A Stop button cancels the running request (abort HTTP / kill child process tree).

1. **openai_compat** (HTTP) — any OpenAI-compatible endpoint. `POST {base_url}/chat/completions`
   with `stream: true`, `Authorization: Bearer <key>`, parse `data:` SSE lines, forward
   `choices[0].delta.content`; also forward `delta.reasoning_content` as event `reasoning`
   (UI shows it collapsed). Sends full conversation history.
2. **anthropic** (HTTP) — `POST https://api.anthropic.com/v1/messages`, headers `x-api-key`,
   `anthropic-version: 2023-06-01`, `stream: true`, `max_tokens` 8192; forward
   `content_block_delta` text deltas. Sends full history.
3. **codex_cli** — spawn `codex exec --json --skip-git-repo-check -C <folder> [-m <model>] -`
   with the prompt on stdin. Parse JSONL events: agent messages -> `delta`, command executions /
   file changes -> `log`. Unknown event types -> `log` with a short summary (never crash on
   unknown JSON). Keep the codex thread/session id from the events so the next message in the
   same chat runs `codex exec resume <id> ...` (continue conversation).
4. **claude_cli** — spawn `claude -p --output-format stream-json --verbose [--model <m>]` with
   cwd=<folder>, prompt on stdin. Assistant text -> `delta`, tool_use -> `log`. Keep `session_id`
   from the result and pass `--resume <id>` for follow-up messages.

CLI notes (Windows): npm-installed CLIs are `codex.cmd` / `claude.cmd`; resolve the executable by
searching PATH for `<name>.exe`, `<name>.cmd`, `<name>` (in that order) and run `.cmd` via
`cmd /C`. Hide the console window (CREATE_NO_WINDOW). If the CLI is not found, return a clear
`error` event, e.g. "Chưa cài Claude Code CLI: npm i -g @anthropic-ai/claude-code".
`GET /api/cli-status` reports which CLIs are installed and their `--version`.

## Config
JSON at `<config_dir>/ai-hub/config.json` (`dirs::config_dir()`; on Windows %APPDATA%).
```json
{ "providers": [ { "id": "...", "name": "Deploy (DeepSeek)", "kind": "openai_compat",
   "base_url": "https://api.deploy.com/api/v1", "api_key": "", "models": ["deepseek-ai/deepseek-v4.1-flash-1017"] } ],
  "default_folder": "C:\\TOOL" }
```
Seed on first run: Deploy (DeepSeek) as above, OpenAI (`https://api.openai.com/v1`, models
`gpt-5.1`), Anthropic (kind anthropic, models `claude-opus-5-5`, `claude-sonnet-5-5`), plus the two
built-in CLI providers (kind `codex_cli`, models `gpt-6-luna`; kind `claude_cli`, models
`opus`, `sonnet`) which need no key.
- `GET /api/config` returns providers with keys **masked** (`sk-…abcd`, or empty), never the raw key.
- `PUT /api/providers/:id`, `POST /api/providers`, `DELETE /api/providers/:id`. On update an empty
  or masked `api_key` value means "keep the existing key".
- `GET /api/providers/:id/models` (openai_compat only): fetch `{base_url}/models` to fill the model list.
- Never log API keys or put them in URLs.

## UI (Vietnamese labels)
- Left sidebar: list of chats (in memory + saved to `<config_dir>/ai-hub/chats/*.json`), "+ Chat mới".
- Top bar of a chat: Nguồn (provider select), Model (select, editable), Thư mục (text input with the
  folder path; validated server-side that it exists).
- Message list with markdown-ish rendering (code blocks with copy button, paragraphs; escape HTML —
  no innerHTML of untrusted text without escaping). Collapsed "Đang suy nghĩ" for reasoning,
  collapsed "Hoạt động" log for CLI tool activity.
- Input box: Enter to send, Shift+Enter newline, Stop button while streaming.
- "Cài đặt" dialog: add/edit/delete providers, paste key, "Tải danh sách model", shows CLI status.
- Dark theme by default, follows `prefers-color-scheme`.

## Out of scope (v1)
Auth, multi-user, remote access, Tauri window, key rotation.
