Step 9 of AI Hub (C:\TOOL\ai-hub) — make Claude Code CLI discoverable and usable. Read the code first.
Build: `CARGO_TARGET_DIR=target-codex cargo build --release --offline`, `cargo clippy --offline --all-targets -- -D warnings`,
`cargo test --offline`, `node --check ui/app.js`. Do not run claude yourself.
ENCODING: write every file as UTF-8 (no PowerShell Set-Content/Out-File). After editing, grep src/ and ui/ for
`\uFFFD`, `Ã`, `á»`, `áº` and for Vietnamese words turned into `?` (e.g. `l?i`, `Ch?y`) and fix them.
Keep code readable (no minified one-liners).

Facts verified by the reviewer on this machine:
- The Claude desktop app ships the real Claude Code CLI at
  `%LOCALAPPDATA%\Claude-Data\claude_<id>\claude-code\<version>\<hash>\claude.exe`
  (two versions present: 2.1.317 and 2.1.309). `claude.exe --version` prints `2.1.317 (Claude Code)`.
  This is NOT under WindowsApps and has no resources\app.asar next to it, so it passes the existing safety rule.
- Standalone it is not logged in: `claude.exe auth status` prints JSON `{"loggedIn": false, "authMethod": "none", ...}`,
  and `-p` returns a result event with `is_error: true`, `result: "Not logged in · Please run /login"`.
  `claude.exe auth login` signs in through the browser.
- When AI Hub is started from a terminal inside the Claude desktop app, the environment contains many host
  variables (`CLAUDECODE`, `CLAUDE_CODE_*`, `CLAUDE_PID`, `CLAUDE_AGENT_SDK_*`, `ANTHROPIC_BASE_URL`, ...).

Tasks:
1. Discovery (cli_support resolve + /api/cli-discover/claude): add the glob
   `%LOCALAPPDATA%\Claude-Data\*\claude-code\*\*\claude.exe` with source "Claude app"; sort by version (semver of the
   `<version>` dir), newest first. Auto-resolution (no explicit path set) should fall back to the newest of these
   when no `claude` is on PATH/npm.
2. Clean environment for claude children (chat, test, auth): remove every env var whose name starts with `CLAUDE`
   or `ANTHROPIC_` before spawning (`Command::env_remove`; iterate std::env::vars()). Codex children unchanged.
3. Login status: provider test for claude_cli runs `claude auth status` (10s timeout, CREATE_NO_WINDOW, cleaned env),
   parses JSON `loggedIn`. Found + logged in -> connected "Đã kết nối · <version>". Found but not logged in ->
   status "error", message "Claude Code chưa đăng nhập", detail = path. If `auth status` fails/unknown, fall back to
   the version check only.
4. Chat: if a claude `result` event has `is_error` and text containing "Not logged in", emit error
   "Claude Code chưa đăng nhập — mở Cài đặt › Claude Code CLI › Đăng nhập".
5. Login button: in Settings, for a claude_cli provider that is found but not logged in, show "Đăng nhập Claude".
   Backend `POST /api/providers/{id}/login` opens a NEW VISIBLE console window running `<claude.exe> auth login`
   (Windows: `cmd /C start "Claude login" "<path>" auth login` built with the same safe argument handling used for
   .cmd shims — the path comes only from the resolver, never from the request body; cleaned env). Return immediately.
   UI: after clicking, show "Hoàn tất đăng nhập trong trình duyệt rồi bấm Kiểm tra kết nối" and poll the provider
   test every 5s for up to 3 minutes; on success toast "✓ Đã kết nối Claude Code CLI".
6. Unit tests: env filter (pure function that, given a list of names, returns those to remove), version-dir sort,
   parsing of `auth status` JSON.
Reply with files changed and verification; state explicitly anything not done.
