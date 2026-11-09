Step 10d2 — finish the parts of 10d that are NOT done. FIRST read .tasks/RULES.md, plans/mutis.md and .tasks/step10d.md
(the full spec). Already done in 10d part 1: worker_rules, verify-fail auto-fix, GET /api/mutis/stats (partial), masking test.

Do ONLY the remaining items, completely:
1. Stats: finish GET /api/mutis/stats per step10d.md item 1 (per provider AND per task kind; normalised top-5 errors).
2. "MutiS" page in the sidebar (button "MutiS", same pattern as Tiện ích): summary cards, per-provider and per-kind tables,
   latest 50 events with filters (kind, provider, ok/failed). No chart libraries.
3. Self-diagnosis per step10d.md item 2: POST /api/mutis/diagnose (SSE), lead role via /api/mutis/roles/resolve fallback,
   JSON contract, "<5 events → Chưa đủ dữ liệu" without AI, findings cards, "Áp dụng" with whitelist + confirm old → new,
   diagnoses saved and listed.
4. Unit tests: stats aggregation from a sample jsonl, diagnosis JSON parsing, setting_change whitelist (accept
   mutis.watchdog_minutes / mutis.roles.big.effort / mutis.routing[2].role; reject mutis.roles.lead.api_key, providers[0].api_key,
   unknown fields, out-of-range values).
5. Make SPEC.md/README.md match what is actually implemented.
Update plans/mutis.md progress log. Reply per RULES.md, listing anything still not done.
