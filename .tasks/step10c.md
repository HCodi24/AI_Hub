Step 10c of AI Hub. FIRST read .tasks/RULES.md. Design: plans/mutis.md (feature name: MutiS) items 5–6. Steps 10a–10b are done.

1. Verify gate (src/verify.rs), effective when orchestration.enabled && verify_gate:
   - `POST /api/verify` {folder} → SSE `log` lines + final `verify` event
     `{ok, summary_lines: [..max 12..], checks: [{name, ok, detail}], duration_ms}`.
   - Command: orchestration.verify_command if non-empty (run via the platform shell with cwd=folder), else auto-detect:
     `scripts/verify/verify.sh` (run with Git Bash `bash` if found) → `Cargo.toml` (cargo build --offline,
     cargo test --offline; clippy only if available) → `package.json` (npm run -s typecheck / lint / build / test when the
     script exists) → nothing ("Không có lệnh kiểm tra cho thư mục này").
   - Always add a built-in MOJIBAKE check over files changed according to `git diff --name-only HEAD` (or all text files
     < 1 MB under src/ ui/ app/ if not a git repo): U+FFFD, `Ã`, `á»`, `áº`, and Vietnamese-word `?` patterns
     (regex like `\b\p{L}{0,4}\?\p{Ll}{1,5}\b` inside string literals). Report file:line (max 10).
   - Timeouts (15 min total), CREATE_NO_WINDOW, kill tree on cancel, strip ANSI, keep only the tail of long outputs;
     summary_lines are what the reviewer and UI show.
2. Review gate (effective auto_review): after a plan task of kind code/test/docs/small_edit ends OK (and verify ran if
   enabled), run the reviewer (orchestration.reviewer_provider_id/model; default first connected of claude_cli,
   anthropic, codex_cli; CLIs with permission "read") with a prompt containing: task title/detail, the task's final
   summary, `git diff` of the folder limited to 30 KB (or the deepseek `diff` event), and the verify summary. The reviewer
   must answer ONLY JSON `{"verdict":"PASS|FAIL","issues":[{"severity":"high|medium|low","file":"","line":0,
   "problem":"","fix":""}],"notes":""}` (reuse the robust extract_json). Store `review` on the task; event `review`.
3. Auto-fix (effective auto_fix): on FAIL with any high/medium issue and fewer than fix_rounds_max rounds used, append a
   new task "Sửa lỗi review: <title>" (kind same as original, assignee = original assignee, depends_on = [original],
   detail = issues formatted), mark original status "cần sửa", and run it automatically; after it passes review, mark the
   original "xong". Event `fix_task`. When rounds are exhausted, status "lỗi" + toast.
4. Plan runner UI: per task show chips "Kiểm tra: ĐẠT/LỖI", "Review: PASS/FAIL (n vấn đề)"; expandable panels for verify
   summary and review issues; a "Chạy kiểm tra" button per plan; a "Review lại" button per task. Add an optional
   "Tự chạy tiếp" switch on the plan (only when orchestration on) that runs the next ready task automatically after a task
   passes the gates (sequential, stop on first failure).
5. Unit tests: auto-detect order, mojibake detector (positive + negative examples incl. `?.` and `??` in JS), review
   JSON parsing, fix-round accounting.
Update plans/mutis.md progress log. Reply per RULES.md.
