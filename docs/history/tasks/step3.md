Step 3 of AI Hub (C:\TOOL\ai-hub). Read SPEC.md and the code. Build with `cargo build --release --offline`
and `cargo clippy --offline`; both must be clean. Do NOT try to run codex/claude yourself (the sandbox
blocks it) — the reviewer already tested live. Real Codex 0.167 JSONL observed on this machine:

{"type":"thread.started","thread_id":"01a1ba34-..."}
{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Codex is ignoring 1 unrecognized configuration setting..."}}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"OK"}}
{"type":"turn.completed","usage":{"input_tokens":17441,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":31,"reasoning_output_tokens":24}}

Fix / add:
1. codex_cli: an `item` of type "error" is a NON-fatal warning (the turn continues). Map it to a `log`
   event prefixed "Cảnh báo: ", not `error`. Only top-level `error` / `turn.failed` are errors.
2. codex_cli: on `turn.completed` emit a `log` "Token: vào X (cache Y), ra Z (suy luận W)" from `usage`.
   claude_cli: on `result` emit a similar log from `usage` and `total_cost_usd` if present.
3. Permissions per chat. Add an optional `permission` field to the chat request and chat record:
   "read" | "write" (default "write"). codex_cli: pass `-s read-only` or `-s workspace-write`
   (check `codex exec resume --help` output in your memory of the CLI; if resume does not accept `-s`,
   use `-c sandbox_mode="..."` which both accept). claude_cli: "write" -> `--permission-mode acceptEdits`,
   "read" -> `--permission-mode plan`. UI: a select "Quyền" next to Thư mục with
   "Chỉ đọc" / "Được sửa file", shown only for CLI providers, saved with the chat.
4. Remove `-C <folder>` from codex args; the child already runs with current_dir = folder. After this,
   the only user-influenced args passed through `cmd /C` are the validated model, permission enum and
   session id. Additionally reject (error event) if the resolved folder path contains `%` or `"`.
5. Use proper Vietnamese with diacritics in all user-facing messages in cli_support.rs
   ("Không thể khởi động", "kết thúc với mã thoát", ...).
6. UI: show the token log line under the assistant message in a muted style (it is a normal log entry,
   just make the "Hoạt động" section show the last token line even when collapsed).
7. README.md: document providers, permission option, where config/chats are stored, and how to install
   Claude Code CLI (`npm i -g @anthropic-ai/claude-code`, then run `claude` once to log in).

Reply with files changed and verification done.
