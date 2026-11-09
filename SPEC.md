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
`error`, `done`, `diff`. A Stop button cancels the running request (abort HTTP / kill child process tree).

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
5. **deepseek_agent** — when MutiS and its worker toggle are enabled, use a tool-calling
   chat completion loop with confined `list_dir`, `read_file`, `edit_file`, and `write_file` tools.
   The worker cannot execute commands and streams a final collapsed diff.

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
- MutiS settings are saved as config field `mutis` (`orchestration` is accepted as a migration alias).
  Its event history is stored in `<config_dir>/ai-hub/mutis/events.jsonl`.

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

## Tiện ích
Trang **Tiện ích** toàn chiều rộng liệt kê catalog extension dạng thẻ, gồm tên, mô tả, liên kết homepage mở tab mới với `rel=noopener`, trạng thái cài đặt/kết nối/ thư mục hiện tại và các thao tác phù hợp. Nhật ký ở cuối trang nhận đầu ra streaming. Mọi thao tác yêu cầu xác nhận lệnh chính xác và phạm vi file có thể sửa; hoàn tất hiển thị ✓/✗ và làm mới trạng thái.

Backend cung cấp `GET /api/extensions?folder=...`, cùng `POST /api/extensions/{id}/actions/{action}` trả SSE `log`, `error`, `done {ok}` và endpoint dừng tiến trình. Catalog Rust giữ executable, đối số phiên bản, thao tác và trạng thái; client chỉ truyền ID và folder. Mỗi extension chỉ chạy một thao tác tại một thời điểm; tác vụ hết hạn sau 10 phút. Trước khi sửa cấu hình, tạo bản sao trong `<config_dir>/ai-hub/backups/`. CodeGraph được catalog với cài đặt npm global, kết nối Codex/Claude, lập chỉ mục folder, nâng cấp và gỡ cài đặt.

## Step 8 — Chia việc (phase 1)
- Planner provider/model are configurable. Planning receives the request, canonical working folder, up to 200 paths (excluding `.git`, `target`, `node_modules`, `.codegraph`), and configured worker IDs/kinds/status hints. The planner is asked for JSON only; the first balanced JSON object is parsed.
- Plans persist as `<config_dir>/ai-hub/plans/<uuid>.json`. Users can edit task title/detail, assignee, dependencies and kind, add/remove tasks, then approve.
- Approved tasks are manually started in task chats. Code/test/docs use write permission; plan/review use read. Dependencies gate the Run button. No automatic chaining.
- The composer offers explicit planning and a dismissible hint for long/large requests; neither runs automatically.

## MutiS
- Settings expose the master toggle, per-feature retry, Codex watchdog/resume, DeepSeek file worker, verify, review, auto-fix, event log, routing, role choices and editable multi-line worker rules. Worker rules are prepended to plan-task prompts only while MutiS is enabled.
- Verify runs after tasks when enabled and scans command output and changed files for UTF-8/mojibake problems. Failure summaries become a fix task on the same assignee when auto-fix has rounds remaining; the fix task is verified and reviewed again.
- Review uses the configured lead source and read-only access. FAIL creates a bounded fix task when enabled.
- Event history is JSONL under `ai-hub/mutis/events.jsonl`; `GET /api/mutis/stats?days=N` summarizes available provider and event-kind aggregates and returns recent events. Stats are limited to retained event history.
- MutiS page provides operational statistics and recent-event filtering. Self-diagnosis sends settings without keys, aggregates and recent failure events to the read-only reviewer. Findings must be JSON and may propose a single whitelisted setting change; applying one requires confirmation and normal MutiS validation. Diagnoses are stored under `ai-hub/mutis/diagnoses/`. With fewer than five events, diagnosis is local and reports insufficient data.
