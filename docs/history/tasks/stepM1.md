Step M1 of AI Hub — "Bộ nhớ dự án" (per-folder markdown memory that the main AI keeps up to date). FIRST read
docs/CODEX_RULES.md and docs/SPEC.md. Build with CARGO_TARGET_DIR=target/codex and `--offline`. Gate: scripts/verify.ps1 +
scripts/smoke.ps1 (smoke only kills the PID it started — run it). No real AI calls. The user runs the app in dev mode from this
repo: never stop ai-hub processes. Another session has UNCOMMITTED edits in ui/index.html, ui/js/chat.js, ui/js/main.js,
ui/js/dev.js, ui/js/plans.js, ui/styles/*.css and ui/components/ — do not revert/reformat them; make only small additive edits there.
Write every file as UTF-8 with LF line endings (never via PowerShell Set-Content/Out-File); Vietnamese text must stay intact.

GOAL (user): in every chat (MutiS or a single AI) the main AI summarises the work into markdown files; the next chat reads them
so it knows where the work stands; when a scope is finished or out of date it is retired automatically. Keep the feature in its
OWN files so it is easy to maintain.

FILES (new): src/features/memory.rs (all logic + unit tests), src/api/memory.rs (HTTP), ui/js/memory.js, ui/styles/memory.css
(import it from ui/styles/main.css). Only thin hooks elsewhere.

1. Storage — `<data_dir>/memory/<project_key>/` where project_key = first 16 hex chars of SHA-256 (or any stable hash already in
   deps; no new crates unless already in Cargo.lock) of the canonical chat folder lower-cased, with the `\\?\` prefix stripped
   (crate::core::text::strip_verbatim). Add `paths::memory_dir(config_path)`. Inside: `<scope>.md` (active scopes),
   `archive/<scope>.md` (retired), `index.md` (regenerated on every change, never hand-edited), `project.txt` (folder path).
   Scope id: `[a-z0-9-]{1,48}`; reject anything else (path safety).
2. Scope file format (YAML-ish frontmatter written/parsed by our own small code, no yaml crate):
   ---
   scope: debate-context
   title: Thêm ngữ cảnh cho Tranh luận
   status: active            # active | blocked | done | obsolete
   updated: 2026-11-10T21:40:00Z
   chats: [<chat ids>]
   files: [src/api/debate.rs]
   next: <one line>
   evidence: [<commit hash or "user-confirmed">]
   ---
   ## Mục tiêu
   ## Đã chốt
   ## Đã làm
   ## Còn mở
   Sections are bullet lists. index.md: one line per active scope `- [status] scope — title · next: … · updated`.
3. Update protocol (cheap: NO extra AI call). Every chat turn, append to the LATEST user message that is sent to the provider
   (never to the stored chat record) a short instruction (≤ ~700 chars, Vietnamese) asking the AI to end its answer with ONE block
       <memory>{"scope":"…","title":"…","status":"active","goal":"…","decisions":["…"],"done":["…"],"open":["…"],
       "next":"…","files":["…"],"evidence":["…"]}</memory>
   or `<memory>none</memory>` when nothing changed; reuse an existing scope id from the index when the topic matches; status
   "done" only with evidence (commit hash, or user said it is finished); "obsolete" when the user abandoned/changed direction.
   After the run finishes (success or not), parse the LAST <memory>…</memory> from the assistant text, REMOVE every such block
   from the stored assistant message content, then merge:
   - new scope → create; existing → merge: decisions & done are append-only with de-dup (never dropped by the AI), open is
     replaced, goal/title/next/files replaced when non-empty, chats gets the chat id, evidence appended.
   - caps: each list ≤ 30 items (drop oldest of done/open first, decisions last), whole file ≤ `max_chars_per_scope`.
   - status done (requires non-empty evidence, else keep active and add an open item "Cần bằng chứng để đóng scope") or obsolete
     → move the file to archive/ (NOT delete). Archived files older than 14 days are deleted when the store is opened.
   - emit an SSE event `memory` {"scope","status","action":"created|updated|archived"} before `done` so the UI can show a toast.
   Invalid JSON → ignore silently (log line "Bộ nhớ: khối <memory> không hợp lệ").
4. Reading protocol. Build `memory_context(folder)`: index.md + the full text of up to `max_scopes_in_prompt` active scopes
   (most recently updated first), wrapped in a header "Bộ nhớ dự án (ghi chú do AI trước để lại — kiểm tra lại với code/git
   trước khi tin)". Inject it:
   - CLI providers (codex_cli, claude_cli): only when the chat has no session_id yet (new chat / switched source), prepended to
     the latest user message in the provider request.
   - other kinds (openai_compat incl. agent mode, anthropic): prepended to the FIRST user message of the provider request copy
     (it is resent with the history every turn anyway).
   - plan generation (/api/plans planner prompt) and debates (proposal_prompt + judge_prompt in features/debate.rs: add an
     optional `memory: &str` parameter) also get it.
   Skip everything when memory is disabled or the folder has no memory.
5. Config: top-level `memory: MemoryConfig { enabled: bool (default true), max_scopes_in_prompt: u32 (default 3, 1..=10),
   max_chars_per_scope: u32 (default 6000, 1000..=20000) }` with serde defaults; round-trips through GET/PUT /api/config like
   other fields (PUT must not reset it when omitted). Dev chats (dev_chat=true) use memory too (their folder is the repo).
6. API (src/api/memory.rs, routes in src/api/mod.rs): GET /api/memory?folder=… → {project, index, scopes:[{scope,title,status,
   updated,next}], archived:[…]}; GET /api/memory/{scope}?folder=… → {text}; PUT same → {text} (user edit; validate frontmatter
   scope matches, regenerate index); POST /api/memory/{scope}/archive?folder=… and /restore?folder=…; DELETE
   /api/memory/{scope}?folder=… (only archived scopes). Folder must exist (400 otherwise).
7. UI (ui/js/memory.js, small hooks only):
   - button "📝 Bộ nhớ" in .composer-actions next to "⚖ Tranh luận"; opens a panel (dialog or the plans view area) listing
     active + archived scopes for the current folder; click → textarea editor with Lưu; buttons Lưu trữ / Khôi phục / Xoá
     (archived only, confirm()).
   - SSE `memory` event in the chat stream → toast "📝 Đã cập nhật bộ nhớ: <scope>" ; when action=archived show
     "Đã đóng scope <scope>" with a "Hoàn tác" link calling /restore.
   - When rendering assistant text (live stream and history) hide `<memory>…</memory>` (and a trailing unfinished `<memory>…`).
   - Settings: a "Bộ nhớ dự án" block with the 3 config fields (checkbox + 2 numbers), saved like other config fields.
8. Tests in features/memory.rs: parse/merge (append-only decisions, de-dup, caps), strip blocks, done-without-evidence stays
   active, done-with-evidence archives, obsolete archives, restore, invalid scope ids rejected, project_key strips \\?\,
   memory_context ordering/limit, archive purge > 14 days, frontmatter round-trip with Vietnamese text.
9. Docs: README section "Bộ nhớ dự án" (Vietnamese, how it works, where files live, cost: ~300–800 output tokens per turn,
   context only on new chat/switch), SPEC section + API list.
Reply per docs/CODEX_RULES.md and list anything not done.
