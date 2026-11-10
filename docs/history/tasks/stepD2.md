Step D2 of AI Hub — one "debate card" for ⚖ Tranh luận inside the chat. FIRST read docs/CODEX_RULES.md and docs/SPEC.md
(sections Tranh luận, Bộ nhớ dự án). Build with CARGO_TARGET_DIR=target/codex and `--offline`. Gate: scripts/verify.ps1 +
scripts/smoke.ps1 (smoke only kills the PID it started — run it). No real AI calls. Never stop ai-hub processes.
Another session has UNCOMMITTED edits in ui/index.html, ui/js/chat.js, ui/js/main.js, ui/js/dev.js, ui/js/plans.js,
ui/styles/*.css and ui/components/ — keep them; touch those files only where this step needs it (debateInChat in chat.js is
yours to replace). Write UTF-8 + LF only (never PowerShell Set-Content/Out-File); keep Vietnamese text intact.

PROBLEM: today `debateInChat` (ui/js/chat.js, used when the "⚖ Tranh luận" switch / localStorage debateMode is on) pushes one
chat message per side per round + judge + verdict. Long answers (~2000 words each) make the chat very long, the two sides stream
interleaved, the verdict is at the bottom, and nothing is saved in the chat record (reload loses it). ui/js/debate.js draws a
second, different view for the debate history page.

GOAL: ONE card per debate, shared by the chat and the history page. New files: ui/js/debate-card.js and
ui/styles/debate-card.css (import from ui/styles/main.css). ui/js/debate.js and debateInChat must both use it; delete their
duplicated rendering code.

1. Card layout (top → bottom):
   - Header: "⚖ Tranh luận · <title>", elapsed timer (mm:ss, live while running), line "Codex <model> · Claude <model> ·
     Trọng tài: Claude mới (<model>)".
   - Progress bar of steps: Vòng 1 Đề xuất → Vòng 2 Phản biện (… up to max_rounds) → Trọng tài → (Chia việc, only if a plan was
     made from it). States: done ●, running ◐ (animated), pending ○, skipped (converged early) shown greyed "bỏ qua".
   - Verdict box FIRST as soon as the verdict exists: "⚖ Phán quyết · <Chọn hướng Codex|Claude|Kết hợp hai hướng>", decision
     text, collapsible chips "✔ Đồng ý (n)", "⚠ Còn tranh cãi (n)", "⚡ Rủi ro (n)", "▸ n bước" (click → expand list), buttons
     "✦ Chia việc từ phán quyết" (GET /api/debates/{id} → planFromRequest(plan_request), and must use the debate's folder — set
     folderInput to debate.folder first if different) and "Xem biên bản" (scrolls/expands rounds).
   - Round tabs [Vòng 1] [Vòng 2]…; selected tab shows TWO columns Codex | Claude (one column with a Codex/Claude toggle under
     700px width). While running, the tab of the running round is auto-selected.
   - Each side box: name, stance badge (✅ Đồng ý / ⚠ Chưa đồng ý, none in round 1), body collapsed to ~6 lines with
     "▸ Xem đầy đủ (N chữ)" / "▾ Thu gọn"; while streaming it shows the tail and auto-scrolls inside the box; markdown rendered
     with the existing renderMarkdown (from chat.js) or plain pre-wrap if importing it would create a cycle problem; "Còn khác: …"
     from disagreements; footer "<tokens> token · <mm:ss>"; while running a pulsing dot + last activity line from `log` events
     (e.g. "Read src/api/debate.rs"); error text in red.
   - Warning chip when one side used > 500000 tokens in a round: "⚠ Codex tốn nhiều token".
   - Footer: "Dừng" button while running (aborts the fetch → server cancels).
   The card must look right in dark and light themes (use existing CSS variables) and at 375px width.
2. API of the module: `createDebateCard({ live: true|false })` returns { el, handle(eventName, data), load(debateRecord) }:
   `handle` consumes the existing SSE events of POST /api/debates (debate, round, delta, log, turn, judging, verdict, error, done);
   `load` renders a stored record from GET /api/debates/{id} (status running → show "đang chạy ở nơi khác / đã bị ngắt" if not
   live). No backend change to the SSE stream is needed — if a field you need is missing, add it compatibly.
3. Chat persistence (backend, small):
   - DebateInput gets optional `chat_id`. When present and valid, the server appends to that chat record (create the record
     if missing, title from the request) a user message (the request text) and an assistant message with a new optional field
     `debate_id` (add `#[serde(default, skip_serializing_if = "Option::is_none")] pub debate_id: Option<String>` to ChatMessage)
     and content = "" at start.
   - When the debate finishes, set that assistant message content to a short markdown summary of the verdict (decision +
     plan steps, ≤ ~1500 chars) or the error, so later chat turns (and the memory feature) see the conclusion. Save the record.
   - Assistant messages with debate_id are NOT sent to providers as normal history except via that summary content
     (already true once content is the summary; ensure an empty one is skipped — the chat handler already drops empty
     assistant turns).
   - debateInChat: send chat_id; render the card inside the chat as one assistant message; renderMessages() must render a
     message with debate_id as a card loaded from GET /api/debates/{id} (cache it in memory; a live card in progress is reused,
     not reloaded). Reloading the page / reopening the chat shows the same card.
4. History page (ui/js/debate.js): list stays; opening a debate renders the same card (live when started from there).
5. Tests: Rust unit tests for appending/updating the chat record (debate_id round-trip, summary content, chat not found →
   created, invalid chat_id → ignored/400 consistently). JS: keep node --check clean.
6. Docs: README (Tranh luận) and SPEC (chat_id, debate_id) short updates.
Reply per docs/CODEX_RULES.md and list anything not done.
