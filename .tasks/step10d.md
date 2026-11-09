Step 10d of AI Hub. FIRST read .tasks/RULES.md. Design: plans/mutis.md (feature name: MutiS) items 7–8. Steps 10a–10c are done.

1. Stats (`GET /api/mutis/stats?days=30`): aggregate events.jsonl per provider/assignee kind and per task kind:
   tasks run, success rate, avg duration, total tokens, retries, watchdog kills, resumes, verify fail rate, review FAIL
   rate, fix rounds, top error messages (normalised, top 5). UI: "MutiS" page (sidebar button "MutiS", like Tiện ích) with
   cards + simple tables (no chart library), plus the latest 50 events with filters (kind, provider, ok/failed).
2. Self-diagnosis "Claude tự đánh giá MutiS":
   - Button "Tự chẩn đoán" on the MutiS page. Backend `POST /api/mutis/diagnose` (SSE logs + final `diagnosis`)
     sends the reviewer (same default chain as review; CLI read-only) a prompt with: current orchestration settings
     (no keys), the stats above, and the last 150 failure-related events (retry, watchdog_kill, resume, verify ok=false,
     review FAIL, fix_task, error, empty DeepSeek answers), each truncated.
   - The reviewer must answer ONLY JSON:
     `{"health":"tốt|trung bình|kém","findings":[{"problem":"","evidence":"","cause":"","severity":"high|medium|low",
       "suggestion":"","setting_change":{"path":"orchestration.<field> or orchestration.routing[i].assignee_kind","value":...}|null}],
       "summary":""}`.
   - Show findings as cards; each finding with a valid `setting_change` gets a button "Áp dụng" that applies ONLY that
     change through the normal PUT /api/orchestration validation (whitelist of fields; reject anything else), after a
     confirm dialog showing old → new value. Save every diagnosis to `<config_dir>/ai-hub/mutis/diagnoses/<ts>.json`
     and list previous ones. Event `diagnosis`.
   - If there are fewer than 5 events, answer locally without calling AI: "Chưa đủ dữ liệu để chẩn đoán".
3. README.md + SPEC.md: document the whole MutiS feature (toggles, routing, DeepSeek làm việc, verify, review,
   auto-fix, stats, tự chẩn đoán) in Vietnamese.
4. Unit tests: stats aggregation from sample jsonl, diagnosis JSON parsing, setting_change whitelist validation.
Update plans/mutis.md progress log. Reply per RULES.md.
