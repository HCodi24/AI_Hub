Step 11b of AI Hub — "Tự phát triển" page. FIRST read .tasks/RULES.md and plans/self-dev.md. Step 11a (dev mode) is done:
src/dev.rs, /api/dev/status, /api/dev/events, /api/dev/rebuild, dev.ps1. Build with CARGO_TARGET_DIR=target-codex as usual.

Only in dev mode (sidebar button "Tự phát triển", next to the DEV badge). Page sections:
1. Repo: path, branch, number of changed files (`git status --porcelain`), last commit (short hash, subject, author),
   refreshed every 10 s while the page is open. `GET /api/dev/repo`.
2. "Sửa app bằng AI": button "Mở chat sửa app" → new chat with folder = repo, source = MutiS if enabled else Codex CLI,
   permission "Được sửa file", composer prefilled with "Sửa AI Hub: "; button "Chia việc cho app" → plan flow with
   folder = repo. Show the hint: "Giao diện tự tải lại; code Rust cần bấm Build & khởi động lại".
3. Git (all commands fixed in code; run in the repo; author from repo git config):
   - "Tạo checkpoint" → `git add -A` + `git commit -m "checkpoint: <ISO time>"` (skip if nothing to commit);
   - "Xem thay đổi" → `git diff HEAD --stat` + full `git diff HEAD` (cap 200 KB) shown in a scrollable pre with
     simple +/- colouring; untracked files listed;
   - "Bỏ thay đổi" → confirm dialog listing files → `git reset --hard HEAD` + `git clean -fd -- src ui` (ONLY those dirs,
     never target/ .tasks/ plans/); show which files were discarded;
   - "Commit" → message input (required) → `git add -A` + `git commit -m <msg>`; show resulting hash. Never push.
   Endpoints under /api/dev/git/*; refuse while AI jobs are active (same guard as rebuild) for reset/commit.
4. Kiểm tra: "Chạy test" → `cargo test` with CARGO_TARGET_DIR=<repo>\target\dev-check (so the running debug build is not
   touched), then the MutiS verify gate on the repo; SSE logs + ✓/✗ summary.
5. "Build exe release" → `cargo build --release` (SSE log); on success show the exe path and a note that a running
   release exe locks the file ("Access is denied" → close it first). Only when the user clicks.
6. Unit tests: the git command builders (exact argv), the clean path whitelist (src, ui only).
Update plans/self-dev.md progress log + README "Chế độ dev" section. Reply per RULES.md.
