Step 10d of AI Hub. FIRST read .tasks/RULES.md. Design: plans/mutis.md (feature name: MutiS) items 7–8. Steps 10a–10c2 are done.

FIX FIRST (from live test of 10c2 — Codex CLI wrote hello.txt with PowerShell as Windows-1252; verify correctly reported
"UTF8: LỖI — hello.txt" but the task just stopped as "lỗi"):
A. Verify failure must also feed auto-fix (when auto_fix on and rounds remain): create "Sửa lỗi kiểm tra: <title>" with the
   verify summary_lines as the issue list, same assignee, run it, then re-verify (+ review). Reuse the pure next-action
   function from 10c2 and extend its tests (verify FAIL → fix; verify FAIL with no rounds → lỗi).
B. MutiS worker rules: new setting `worker_rules` (multi-line text, default:
   "- Ghi mọi file bằng UTF-8 (không dùng PowerShell Set-Content/Out-File/>); dùng công cụ sửa file hoặc [System.IO.File]::WriteAllText với UTF8.
- Không xoá tính năng có sẵn khi viết lại code.
- Báo cáo rõ phần chưa làm.")
   prepended to every plan-task prompt sent to big/small workers and fix tasks when MutiS is on. Editable in Settings › MutiS.
C. Security regression test: role resolve and every endpoint returning providers must never contain a raw api_key
   (reviewer already fixed resolve_mutis_roles by masking — add a unit test for that masking helper).



1. Stats (`GET /api/mutis/stats?days=30`): aggregate events.jsonl per provider/assignee kind and per task kind:
   tasks run, success rate, avg duration, total tokens, retries, watchdog kills, resumes, verify fail rate, review FAIL
   rate, fix rounds, top error messages (normalised, top 5). UI: "MutiS" page (sidebar button "MutiS", like Tiện ích) with
   cards + simple tables (no chart library), plus the latest 50 events with filters (kind, provider, ok/failed).
2. Self-diagnosis "Claude tự đánh giá MutiS":
   - Button "Tự chẩn đoán" on the MutiS page. Backend `POST /api/mutis/diagnose` (SSE logs + final `diagnosis`)
     sends the reviewer (same default chain as review; CLI read-only) a prompt with: current MutiS settings
     (no keys), the stats above, and the last 150 failure-related events (retry, watchdog_kill, resume, verify ok=false,
     review FAIL, fix_task, error, empty DeepSeek answers), each truncated.
   - The reviewer must answer ONLY JSON:
     `{"health":"tốt|trung bình|kém","findings":[{"problem":"","evidence":"","cause":"","severity":"high|medium|low",
       "suggestion":"","setting_change":{"path":"mutis.<field> or mutis.roles.<lead|big|small>.<provider_id|model|effort> or mutis.routing[i].role","value":...}|null}],
       "summary":""}`.
   - Show findings as cards; each finding with a valid `setting_change` gets a button "Áp dụng" that applies ONLY that
     change through the normal PUT /api/mutis validation (whitelist of fields; reject anything else), after a
     confirm dialog showing old → new value. Save every diagnosis to `<config_dir>/ai-hub/mutis/diagnoses/<ts>.json`
     and list previous ones. Event `diagnosis`.
   - If there are fewer than 5 events, answer locally without calling AI: "Chưa đủ dữ liệu để chẩn đoán".
3. README.md + SPEC.md: document the whole MutiS feature (toggles, routing, DeepSeek làm việc, verify, review,
   auto-fix, stats, tự chẩn đoán) in Vietnamese.
4. Unit tests: stats aggregation from sample jsonl, diagnosis JSON parsing, setting_change whitelist validation.
Update plans/mutis.md progress log. Reply per RULES.md.
