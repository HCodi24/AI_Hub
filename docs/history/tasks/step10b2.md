Step 10b2 of AI Hub. FIRST read .tasks/RULES.md. Design: plans/mutis.md (feature name: MutiS). Steps 10a–10b are done.

Problem (from the user): the mode chip/popover next to the composer shows ONE source + model + effort. When MutiS uses
three AIs it cannot show or choose them.

1. MutiS roles in config (`mutis.roles`, serde default derived from existing planner/reviewer/routing settings):
   ```
   roles: {
     lead:  {provider_id, model, effort},   // "Lập kế hoạch & review" (planner + reviewer + self-diagnosis)
     big:   {provider_id, model, effort},   // "Làm việc lớn"  (routing kinds code, test)
     small: {provider_id, model, effort},   // "Làm việc nhỏ"  (routing kinds small_edit, docs)
   }
   ```
   Defaults: lead = first connected of claude_cli, anthropic, codex_cli (model opus / claude-opus-5-5 / gpt-6-luna);
   big = codex_cli gpt-6-luna effort high; small = the deepseek_agent provider. Keep backward compatibility: the planner
   and reviewer settings read from roles.lead (migrate old planner_provider_id/model and reviewer_* into lead on load);
   routing rules keep `kind -> role` ("lead|big|small") instead of assignee kinds (migrate: deepseek_agent→small,
   codex_cli→big, claude_cli→lead). reassign()/plan prompt use the role's provider. Validate effort values as in chat.
2. Mode chip when MutiS is enabled AND selected as source: `[●●● MutiS · <lead model> · <big model> · <small model> ⌃]`
   (prettyModel names, three status dots coloured from the provider connection cache; tooltip lists role → provider ·
   model · effort). When MutiS is off, behaviour unchanged.
3. Popover:
   - "Nguồn" list gets a first item "✦ MutiS (3 AI)" (only when MutiS master switch is on; otherwise a disabled row
     "✦ MutiS — bật trong Cài đặt").
   - When MutiS is selected, replace the Model / Mức suy luận sections with "Đội MutiS": three rows (icon + role name,
     provider select, model select incl. "Model khác…", effort select only if the provider kind supports effort,
     status dot). If a role's provider is not connected show "⚠ chưa kết nối — sẽ dùng <fallback>" using the same fallback
     chain as reassign. Changes save to /api/mutis immediately.
   - Below: quick switches "Kiểm tra tự động", "Claude review", "Tự sửa lỗi" bound to verify_gate / auto_review / auto_fix.
   - Keyboard accessible like the existing popover; closes on Esc/outside click.
4. Sending in MutiS mode: pressing Enter/Gửi with source MutiS calls the existing plan flow ("Gợi ý chia việc") with
   the lead role as planner instead of a normal chat, and opens the plan for approval. The composer placeholder becomes
   "Mô tả việc cần làm — MutiS sẽ chia việc cho 3 AI…". Remember the last selected source (MutiS or a provider) per chat.
5. Settings › MutiS shows the same three role rows (single source of truth with the popover).
6. Unit tests: config migration (old planner/reviewer/assignee-kind routing → roles), role fallback selection.
Update plans/mutis.md progress log. Reply per RULES.md.

Also fix (from review of 10b): deepseek_agent logs "Sửa ../escape.txt" BEFORE the path check rejects it, which looks like
the edit happened. Emit the progress log only after validation succeeds; on rejection log "Từ chối: <path> (<reason>)".
