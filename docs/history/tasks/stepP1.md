Step P1 of AI Hub — a third permission level "Toàn quyền" so CLI agents can RUN COMMANDS. FIRST read docs/CODEX_RULES.md.
Repo is now C:\GitHub\AI_Hub. Build with CARGO_TARGET_DIR=target/codex and `--offline`. Gate: scripts/verify.ps1 + scripts/smoke.ps1
(smoke only kills the PID it started). No real AI calls. Never stop ai-hub processes. UTF-8 + LF only (never PowerShell
Set-Content/Out-File); keep Vietnamese intact. Do NOT touch or delete the untracked file _restructure_ui.py (another session's).

PROBLEM (user screenshot): in a chat with Claude Code CLI and permission "Được sửa file", Claude answered "mọi lệnh sửa hay chạy
script (python, git mv) đều bị chặn chờ duyệt ... requires approval". Cause: src/providers/claude_cli.rs maps Permission::Write →
`--permission-mode acceptEdits`, which auto-approves file edits only; in `-p` mode any Bash/PowerShell command is auto-denied
because nobody can approve it. Codex (workspace-write sandbox) can already run commands.
Also the token line shows a float artifact: "chi phí $0.5748474000000001" (src/providers/cli_support.rs ~line 348).

DO
1. Add `Permission::Full` (serde "full") to the enum in src/features/chat.rs: meaning "sửa file + chạy lệnh trong thư mục".
   Keep Write as the default. Update every match (compiler will list them).
   - claude_cli: Read → "plan"; Write → "acceptEdits" (unchanged); Full → `--permission-mode acceptEdits` PLUS
     `--allowedTools Bash PowerShell` (pass each tool name as its own arg after the flag, verify against `claude --help` text
     in your knowledge; if unsure use `--allowedTools "Bash,PowerShell"`), never bypassPermissions.
   - codex_cli: Read → read-only; Write → workspace-write (unchanged); Full → workspace-write as well (Codex already runs
     commands there) — add a comment.
   - other providers (openai_compat agent mode, anthropic): Full behaves like Write (agent mode never runs commands; say so).
   - MutiS/plan tasks: task.permission accepts "full"; worker permission mapping (taskCanWrite etc. in ui/js/plans.js /
     plan-card.js / plan-helpers.js) treats full as write-capable.
2. UI: add `<option value="full">Toàn quyền (chạy lệnh)</option>` to every permission select (ui/index.html composer select,
   plan-card task select, debate-actions "Thực hiện với…" select, any other — grep `Được sửa file`). Tooltip/title: "AI được
   sửa file và chạy lệnh (python, git, cargo…) trong thư mục làm việc. Chỉ dùng với thư mục bạn tin tưởng." Mode chip summary
   shows "Toàn quyền" when selected. For providers that cannot run commands (not codex_cli/claude_cli) show the option disabled
   or a note "Nguồn này không chạy lệnh".
3. When Claude CLI reports a permission denial for a command while permission != full, surface a clear hint in the chat log:
   detect tool_result errors containing "requires approval" / "haven't granted" in the Claude stream parser and emit a log line
   "Lệnh bị chặn: chọn Quyền › Toàn quyền (chạy lệnh) để cho phép AI chạy lệnh." (once per run).
4. Cost formatting: show at most 4 decimals, trimmed ("$0.5748"), for numbers and numeric strings. Unit test.
5. Tests: claude args for Read/Write/Full (Full contains --allowedTools and acceptEdits, never bypassPermissions), codex args for
   Full, serde round-trip "full", cost formatting, denial hint emitted once.
6. Docs: README (Quyền: Chỉ đọc / Được sửa file / Toàn quyền) + SPEC.
Reply per docs/CODEX_RULES.md and list anything not done.
