Step 11a of AI Hub — dev mode (run from source, the running app edits itself). FIRST read .tasks/RULES.md and
plans/self-dev.md (the design; the old "test exe on port 7879 / swap exe" idea is CANCELLED — do not implement it).

1. CLI flag `--dev` (and env AI_HUB_DEV=1). Repo root in dev mode: env AI_HUB_REPO if set, else walk up from the
   current exe until a Cargo.toml with `name = "ai-hub"` is found; abort with a clear message if none.
2. UI from disk in dev mode: the handlers for /, /app.js, /style.css read `<repo>\ui\<file>` on every request
   (Cache-Control: no-store); release/non-dev builds keep the embedded include_str! versions.
3. Live reload: a background task polls mtimes every 1s of `<repo>\ui\*` and of `<repo>\src\**\*.rs` + Cargo.toml +
   Cargo.lock. SSE `GET /api/dev/events` sends `ui_changed` and `rust_changed {files:[...]}`. app.js (only when the
   server says dev mode via GET /api/dev/status) subscribes: `ui_changed` → reload the page (keep state minimal: the
   current chat id is already restorable); `rust_changed` → show a top banner "Code Rust đã thay đổi (n tệp) ·
   [Build & khởi động lại]". If the SSE connection drops (server restarting), retry every 1s and reload when it is back.
4. `POST /api/dev/rebuild` (SSE): refuse with 409 "Đang có N việc AI chạy — chờ xong hoặc dừng" if any chat/plan/
   extension/diagnosis job is running (keep a counter of active jobs in AppState). Otherwise run `cargo build` in the repo
   (target\debug; the running binary is a copy in target\dev-run so the file is not locked), stream logs (strip ANSI,
   tail), final `rebuild {ok, duration_ms, errors_tail}`. On success: send the event, wait ~300 ms, then exit the process
   with code 75. On failure: keep running, show the compiler error tail in the banner.
   Setting `self_dev.auto_rebuild` (default false, in Settings › Tự phát triển, only shown in dev mode): when true, a
   `rust_changed` with no active jobs triggers the rebuild automatically after 3 s of no further changes.
5. Launcher scripts in the repo root:
   - `dev.ps1`: loop { `cargo build`; if it fails and target\dev-run\ai-hub.exe exists, print the error and run the old
     copy; copy target\debug\ai-hub.exe → target\dev-run\ai-hub.exe; run it with `--dev` plus any args passed to dev.ps1
     (e.g. -port 7878) and wait; if exit code != 75 break }. Print short Vietnamese status lines. Ctrl+C stops.
   - `dev.cmd`: `powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev.ps1" %*`.
   Keep target/ in .gitignore (already). Write both files as UTF-8 (dev.ps1 with BOM so Windows PowerShell 5.1 reads
   Vietnamese correctly).
6. UI: a "DEV" badge in the sidebar when in dev mode; the rebuild banner with live log, error tail, and a spinner
   "Đang khởi động lại…" while the server is down.
7. README: section "Chế độ dev (tự phát triển)" — how to start (`.\dev.ps1`), what reloads automatically, Rust rebuild,
   that it uses the real data, and that building the release exe is a separate manual step.
8. Unit tests: repo root discovery (walk-up + name check), mtime change detection (pure function over two snapshots).
Update plans/self-dev.md progress log. Reply per RULES.md.
