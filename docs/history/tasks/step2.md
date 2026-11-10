You are implementing step 2 of the AI Hub app in C:\TOOL\ai-hub. Read SPEC.md and the existing code first.
Step 1 (server, config, HTTP providers, UI) is already done; keep its structure.

## Part A — fix review findings from step 1 (do these first)
Dependencies are now cached and Cargo.lock exists: build with `cargo build --release --offline`
(and `cargo clippy --offline`); the sandbox has no reliable network.
1. Compile error E0382 in src/server.rs:439 — `drop(provider_sender)` after it was moved into the spawned
   task. Remove that line (the provider task owns the sender; the channel closes when it finishes).
2. Unused import `delete` in server.rs.
3. `persist_event` rewrites the chat JSON file on EVERY streamed token. Throttle it: keep updating the
   in-memory record, but write to disk at most once per second plus on log/error and at the end.
4. openai_compat.rs / anthropic.rs: on non-2xx, include the response body (first ~500 chars, keys never
   included) in the error so the user sees e.g. "model not found". Also surface `{"error": ...}` objects
   that arrive inside the SSE stream as an `error` event.
5. DNS-rebinding protection: add a middleware that rejects requests whose `Host` header is not
   `127.0.0.1:<port>` or `localhost:<port>`, and rejects non-GET requests whose `Origin` header is present
   and not one of those origins (403).

## Part B — Step 2 scope: implement the two CLI providers in `src/providers/codex_cli.rs` and `src/providers/claude_cli.rs`
and wire them into the existing Provider abstraction, replacing the "Chưa hỗ trợ ở bước 1" stub.

Requirements:
1. Executable resolution per SPEC.md (PATH search: .exe, .cmd, bare; `.cmd` via `cmd /C`), hidden window
   on Windows (CREATE_NO_WINDOW via `std::os::windows::process::CommandExt`, cfg(windows) only).
   IMPORTANT: when running through `cmd /C`, do NOT put the user prompt on the command line (quoting/injection
   risk) — always send the prompt on stdin, and pass only fixed flags, model and folder as separate args.
   Validate model names against `^[A-Za-z0-9._:/-]+$` before passing them.
2. codex_cli: `codex exec --json --skip-git-repo-check -C <folder> [-m <model>] -` (prompt on stdin).
   Follow-up messages in the same chat: `codex exec resume <thread_id> --json --skip-git-repo-check [-m <model>] -`
   (check `codex exec resume --help` locally for the exact flag order supported by the installed version
   and adapt). Discover the real JSONL event shapes by running a tiny harmless prompt yourself, e.g.
   `echo "Say hi, do not run commands" | codex exec --json --skip-git-repo-check -` in a temp dir, and
   map: agent message text -> delta, reasoning -> reasoning, command/file-change/tool items -> log
   (short one-line summaries), thread/session id -> remember in the chat store, errors -> error.
3. claude_cli: `claude -p --output-format stream-json --verbose [--model <m>] [--resume <session_id>]`,
   cwd=<folder>, prompt on stdin. Map assistant text blocks -> delta, tool_use -> log ("Read file x",
   "Bash: cmd"), `result` event -> remember session_id, is_error -> error. Claude CLI is NOT installed on
   this machine: implement from the documented stream-json format (types system/assistant/user/result,
   assistant.message.content[] with text / tool_use blocks) and make missing-CLI return the friendly error.
4. Only the newest user message is sent to a CLI (the CLI keeps its own history via resume). If no session
   id is stored yet but the chat has earlier messages, prepend a short transcript of the earlier messages.
5. Cancellation: Stop / client disconnect must kill the whole child process tree (on Windows use
   `taskkill /PID <pid> /T /F`; elsewhere kill the child). No zombie processes.
6. Collect stderr; if the process exits non-zero, send an `error` event with the last ~20 stderr lines.
7. Persist the CLI session id in the chat JSON so follow-ups work after restarting the app.
8. UI: show "Hoạt động" log entries live; show CLI status in settings (already there — verify it works).

Verify: `cargo build --release` and `cargo clippy` with no errors. Then actually start the server
(`target/release/ai-hub --no-open --port 7899`), POST a chat to the codex_cli provider with a harmless
prompt ("Trả lời đúng một từ: OK. Không chạy lệnh nào.") in a temp folder via curl, confirm you get
delta events and done, then stop the server.

When finished, reply with: files changed, the real codex JSONL event types you observed, how you verified,
and anything unfinished.
