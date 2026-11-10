Step 11a of AI Hub — "Tự phát triển" (self-dev). FIRST read .tasks/RULES.md and plans/self-dev.md (the design).

1. Config `self_dev` (serde default): { enabled: false, repo_path: "" (empty = auto-detect), dev_port: 7879,
   dev_target_dir: "target-dev" }. Auto-detect repo: from std::env::current_exe(): if it is `<X>\target\release\ai-hub.exe`
   (or target-*\release) and `<X>\Cargo.toml` contains `name = "ai-hub"` → repo = X. GET/PUT `/api/self-dev/config`.
   Settings gets a section "Tự phát triển" (toggle + repo path + dev port).
2. `GET /api/self-dev/status`: repo path, detected?, git branch, changed file count (`git status --porcelain`), last commit
   (hash, subject, author), current exe path, whether a dev build exists (path + mtime), whether the dev instance is running
   (pid, port), last build/test/verify results (kept in memory).
3. `POST /api/self-dev/build` (SSE): run in the repo with env CARGO_TARGET_DIR=<repo>\<dev_target_dir>:
   `cargo build --release` then `cargo test` (same target dir), then the existing verify gate on the repo (reuse
   crate::verify::run with the MutiS verify settings). Stream logs (strip ANSI, tail long output), final event
   `build` {ok, steps:[{name, ok, duration_ms, tail}], exe_path}. Only one build at a time (409). Cancel kills the tree.
4. Dev instance: `POST /api/self-dev/run` starts `<repo>\<dev_target_dir>\release\ai-hub.exe --port <dev_port> --no-open`
   with env AI_HUB_CONFIG_PATH=<config_dir>\ai-hub\dev-sandbox\config.json. Before first start (or when the user clicks
   "Làm mới dữ liệu thử") copy the real config.json there (keys included — it stays on this machine) — never copy chats.
   Capture its stdout/stderr to a ring buffer (GET `/api/self-dev/logs`). Wait until it answers GET / (timeout 15s) →
   return {url}. `POST /api/self-dev/stop` kills it. Kill it on app exit. Refuse to run if no dev build exists.
   Never run two dev instances.
5. UI: sidebar button "Tự phát triển" (only when enabled) → page with cards:
   - Dự án (path, branch, changed files, last commit; buttons "Mở chat sửa app" → new chat with folder = repo,
     source = MutiS if enabled else Codex CLI, permission "Được sửa file"; "Chia việc cho app" → open plan flow with folder = repo);
   - Bản thử: "Build bản thử" (live log, per-step ✓/✗), "Chạy bản thử" / "Dừng", link "Mở bản thử ↗" (new tab),
     "Làm mới dữ liệu thử", dev instance log viewer.
   - A note: "Áp dụng bản mới và khôi phục sẽ có ở bước 11b".
6. Security: all commands are fixed in code (cargo/git with fixed args); repo_path must contain Cargo.toml with
   name = "ai-hub"; the browser never sends command text. Host/Origin middleware already applies.
7. Unit tests: repo auto-detect from exe path, Cargo.toml name check, sandbox path derivation.
Update plans/self-dev.md progress log + README (short section "Tự phát triển"). Reply per RULES.md.
