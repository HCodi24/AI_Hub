Step 8 of AI Hub (C:\TOOL\ai-hub) — "Chia việc" (task planning), phase 1. Read SPEC.md and the code first.
Build with `CARGO_TARGET_DIR=target-codex cargo build --release --offline` and clippy; both clean.
Do not run real AI calls yourself.

Entry points (the "suggest a split" button)
1. Composer: next to the send button add a button "✦ Gợi ý chia việc" (icon + tooltip). It takes the text in
   the message box (or, if empty, the last user message of the chat) and asks the PLANNER to propose a task plan.
   Also show a subtle inline hint above the composer when the typed message is long (> 300 chars) or contains
   words like "làm app", "xây dựng", "thêm tính năng", "refactor": "Yêu cầu này khá lớn — ✦ Gợi ý chia việc?"
   (dismissible, never auto-runs).
2. Sidebar: a "Chia việc" view listing saved plans.

Planner
3. Settings gets "Người lập kế hoạch": any configured provider+model (default: first connected of
   claude_cli, anthropic, codex_cli; codex_cli runs with permission "read"). The planner gets: the request,
   the working folder path, a short file tree of the folder (max 200 entries, skip .git/target/node_modules/
   .codegraph), and the list of available workers with their connection status. It must answer ONLY with JSON:
   {"title": "...", "tasks": [{"id": 1, "title": "...", "detail": "files to touch + expected result",
     "assignee": "codex_cli|deepseek|claude|...provider id", "depends_on": [ids], "kind": "plan|code|test|docs|review"}]}
   Guidance in the prompt: Claude/planner for design & review, Codex for multi-file code, DeepSeek (Deploy) for
   tests-from-a-given-list/docs/mechanical edits; small tasks; no two tasks editing the same file in parallel.
   Parse robustly (extract the first JSON object; on failure show the raw answer + "Thử lại").

Plan editor (user must approve before anything runs)
4. Show the plan as a table: #, task, giao cho (select of providers), phụ thuộc, loại, trạng thái. Rows are
   editable (title/detail textarea, assignee, depends_on), can be added/deleted/reordered. Buttons:
   "Lập lại kế hoạch", "Duyệt kế hoạch". Plans persist to `<config_dir>/ai-hub/plans/<id>.json`.
5. After approval each row gets "Chạy" (enabled only when its dependencies are done). Running a task opens/uses a
   chat bound to that task with the assignee provider, folder, permission "write" for code/test/docs tasks and
   "read" for plan/review, and a prompt built from: plan title, this task's detail, summaries of finished
   dependency tasks. Show live log + token line in the row; states: chờ / đang chạy / xong / lỗi / đã dừng.
   Buttons per row: Dừng, Chạy lại, Đánh dấu xong. No automatic chaining in phase 1.
6. Keep per-task token totals and show a plan total.

Update README.md and SPEC.md. Reply with files changed and verification.
