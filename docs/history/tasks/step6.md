Step 6 of AI Hub (C:\TOOL\ai-hub). Read SPEC.md and the current code (step 5 mode bar is done).
Build with `CARGO_TARGET_DIR=target-codex cargo build --release --offline` and
`CARGO_TARGET_DIR=target-codex cargo clippy --offline`; both clean. Do not run codex/claude for real chats.

Goal: in the Settings dialog ("Nguồn AI" pills: Deploy (DeepSeek), OpenAI, Anthropic, Codex CLI,
Claude Code CLI), every source gets a connection status, and the CLI sources get a one-click
"Tự tìm trên máy" button.

A. Connection status for EVERY provider
1. Each provider pill shows a small status dot: green = "Đã kết nối", red = "Lỗi kết nối",
   grey = "Chưa kiểm tra" / "Chưa cấu hình". Tooltip shows the detail text.
2. In the provider's edit panel add a status line + a button "Kiểm tra kết nối".
   Backend `POST /api/providers/{id}/test` returns `{ ok, status: "connected"|"error"|"not_configured",
   message, detail }` (never include keys):
   - openai_compat: GET {base_url}/models with the key (timeout 15s). ok -> message
     "Đã kết nối · N model". 401/403 -> "Sai API key". 404 -> "Sai Base URL". Network -> "Không kết nối được".
   - anthropic: GET https://api.anthropic.com/v1/models with x-api-key + anthropic-version (timeout 15s).
   - codex_cli / claude_cli: resolve the executable and run `--version` (timeout 10s);
     ok -> "Đã kết nối · <version> · <path>".
   - Empty key / no executable -> not_configured "Chưa cấu hình".
3. When the Settings dialog opens, test all providers in parallel automatically (show a spinner per pill),
   and re-test a provider after it is saved. Cache results in memory (UI) for the session.
4. When a test turns green after it was not green, show a toast "✓ Đã kết nối <tên nguồn>" (auto-hide 3s);
   a new red result shows a toast "✗ <tên nguồn>: <message>".
5. The mode chip near the chat input (step 5) also shows the same coloured dot for the current provider.

B. "Tự tìm trên máy" for codex_cli and claude_cli
6. Backend `GET /api/cli-discover/{codex|claude}` scans and returns a list of candidates
   `[{ path, version, source }]` (source = "PATH", "npm", "Codex app", "registry PATH", ...), deduplicated
   (same version + same underlying install -> keep the shortest/most canonical path, prefer the `.cmd`
   shim in %APPDATA%\npm, then app-bundled `.exe`). Search the same places as `resolve_executable` plus:
   `%LOCALAPPDATA%\OpenAI\Codex\bin\*\codex.exe`,
   `%APPDATA%\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\*\bin\codex.exe`,
   `%LOCALAPPDATA%\Packages\*\LocalCache\Roaming\npm\{codex,claude}.cmd`,
   `%USERPROFILE%\.local\bin\claude.exe`, `%USERPROFILE%\.claude\local\claude(.exe|.cmd)`,
   `%APPDATA%\npm\claude.cmd`. Bounded: no recursive disk-wide scan, max ~200 dir checks, overall timeout 20s.
7. SAFETY: never execute anything under `C:\Program Files\WindowsApps\` or any file named `Claude.exe` whose
   directory also contains `resources\app.asar` — that is the Claude DESKTOP app, not the CLI, and running
   it would open a GUI window. Only accept a candidate if `--version` exits within 10s and its output matches:
   codex -> contains "codex" (case-insensitive); claude -> contains "Claude Code". Run version checks with
   CREATE_NO_WINDOW and kill on timeout.
8. UI: in the codex_cli / claude_cli edit panel, next to "Đường dẫn file chạy", add a button
   "Tự tìm trên máy". Click -> spinner -> if 1 candidate: fill the path field, save, test, toast
   "✓ Đã tìm thấy và kết nối Codex CLI (<version>)". If several: show a small list (path, version, source)
   to pick one. If none: show "Không tìm thấy" + the install hint
   (codex: `npm i -g @openai/codex`; claude: `npm i -g @anthropic-ai/claude-code`) with a copy button.

Reply with files changed and verification done.
