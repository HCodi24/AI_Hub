You are implementing step 1 of the AI Hub app in the current directory (C:\TOOL\ai-hub).
Read SPEC.md first; it is the source of truth.

Step 1 scope:
1. Create the Cargo project here (package name `ai-hub`, binary `ai-hub`). Add `.gitignore` (target/).
2. Server: axum on 127.0.0.1, port flag/env, opens browser, embedded UI (index.html, app.js, style.css).
3. Config module: load/seed/save config.json exactly as in SPEC.md, masked keys in GET, provider CRUD
   endpoints with "empty or masked key keeps existing key", GET /api/providers/:id/models.
4. Chat endpoint `POST /api/chat` (body: chat_id, provider_id, model, folder, messages[]) returning SSE
   with events delta / reasoning / log / error / done. Implement providers `openai_compat` and
   `anthropic` fully. For `codex_cli` and `claude_cli` return an `error` event "Chưa hỗ trợ ở bước 1"
   (step 2 will implement them) — but design a `Provider` abstraction so CLIs plug in cleanly,
   including cancellation (client disconnect or `POST /api/chat/:chat_id/stop` must abort work).
5. Chat persistence to `<config_dir>/ai-hub/chats/*.json` + list/get/delete endpoints.
6. Full UI as described in SPEC.md (settings dialog, chat list, streaming rendering, stop button,
   safe escaping).
7. `GET /api/cli-status` (just detection + `--version`, using the PATH resolution rules in SPEC.md).

Constraints:
- Must compile with `cargo build --release` with zero errors; fix all warnings you reasonably can.
- Run `cargo build --release` and `cargo clippy` (if available) yourself and fix issues.
- Do not hard-code any real API key. Never print keys.
- Keep code split into modules: main.rs, config.rs, server.rs (routes), chat.rs (SSE + store),
  providers/{mod.rs, openai_compat.rs, anthropic.rs}.
- Add a short README.md in Vietnamese: build, run, where config is stored.

When finished, reply with: files created, how you verified the build, and anything left unfinished.
