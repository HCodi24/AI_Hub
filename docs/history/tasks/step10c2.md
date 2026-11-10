Step 10c2 — fix live-test findings of 10c. FIRST read .tasks/RULES.md. Design: plans/mutis.md.

Reviewer ran a real plan in the browser (temp git repo, one code task "Tạo file hello.txt chứa: xin chào" → Codex CLI,
MutiS on with verify_gate + auto_review + auto_fix; Codex connected; Claude CLI not logged in; no API keys):
1. Review returned FAIL "Không có reviewer MutiS kết nối" (severity high) although Codex is connected. The plans page never
   ran provider tests, so statuses were unknown. Fix: before reviewing (and before planning/running), resolve the lead
   role SERVER-SIDE using the provider test + 60s cache (add `GET /api/mutis/roles/resolve` returning, per role, the
   configured provider, its live status, and the provider actually used). Use that in the UI runner.
2. When no reviewer can be resolved: do NOT mark FAIL/lỗi and do NOT consume a fix round. Record
   review = {verdict:"SKIPPED", notes:"Không có nguồn review đang kết nối"}, chip "Review: bỏ qua", task status "xong"
   (verify result still applies: if verify failed the task is "lỗi").
3. Auto-fix did not create a fix task (fix_rounds stayed null). After fixing 1, make sure a FAIL with high/medium issues
   creates "Sửa lỗi review: …" and runs it; add a unit/integration-style test of the state transitions
   (pure function: (task, review, rounds_used, max) -> next action).
4. VERIFY MISSED THE ENCODING BUG: Codex wrote hello.txt with PowerShell Set-Content → bytes are Windows-1252 ("xin ch\xE0o"),
   the file shows "xin ch�o". MOJIBAKE reported "Không phát hiện" because it only scanned `git diff --name-only HEAD`
   (untracked new files are excluded) and only looked for mojibake patterns. Fix:
   - files to scan = `git diff --name-only HEAD` ∪ `git ls-files --others --exclude-standard` (cap 500 files, skip > 1 MB
     and binary by extension/NUL byte);
   - new check "UTF8": any scanned text file that is not valid UTF-8 → LỖI with file names (this is the main failure mode);
   - keep the mojibake patterns check.
5. False positive: verifying C:\TOOL\ai-hub flags `.tasks\step10c.md:17`, a doc that QUOTES example mojibake. Add a MutiS
   setting `verify_ignore` (list of glob patterns, default [".tasks/**", "plans/**", "**/*.md"]) applied to MOJIBAKE only
   (UTF8 check still applies to everything scanned). Editable in Settings › MutiS.
Unit tests for 2, 3, 4 (invalid UTF-8 bytes, untracked file included), 5 (ignore globs).
Update plans/mutis.md progress log. Reply per RULES.md.
