Step 10b of AI Hub. FIRST read .tasks/RULES.md. Design: plans/mutis.md (feature name: MutiS) items 3–4. Step 10a (orchestration
config, event log, retry, watchdog) is done — reuse it.

FIX FIRST (bug found in live test of 10a): Codex auto-resume works for round 1 but round 2 fails with
`Codex CLI kết thúc với mã thoát 2: error: unexpected argument '<thread-id>' found` — the args for the second
`codex exec resume` are built wrong (likely the session id / "resume" pushed twice or the args vector reused across rounds).
Build a fresh args vector per round from one function `codex_args(session_id: Option<&str>, model, effort, permission)`
and unit-test that rounds 1 and 2 produce identical, valid `exec resume [OPTIONS] <id> -` argv. Also: only check the
LAST paragraph of the final agent message for the stop-early phrases (a reply that merely quotes the word "incomplete"
earlier should not trigger).

0. RENAME — the feature is the user's own feature named **MutiS** (exact spelling, capital M and S).
   - Module `src/orchestration.rs` → `src/mutis.rs`; Config field `orchestration` → `mutis` with
     `#[serde(alias = "orchestration")]` so existing config.json keeps working; types `Orchestration*` → `Mutis*`.
   - Endpoints `/api/mutis`, `/api/mutis/events` (keep `/api/orchestration*` as aliases for one release).
   - Data dir `<config_dir>/ai-hub/mutis/` (move `orchestration/events.jsonl` there on first start if present).
   - UI: every user-visible "Điều phối" / "Điều phối thông minh" → "MutiS". Settings section title "MutiS", master toggle
     "Bật MutiS" with subtitle "Claude lập kế hoạch & review · Codex làm việc lớn · DeepSeek làm việc nhỏ", sidebar badge
     "MutiS: BẬT/TẮT", messages like "Bật MutiS › DeepSeek làm việc trên file để dùng nguồn này".
   - Update references to plans/orchestration.md in SPEC.md/README.md to plans/mutis.md (file already renamed).
   In everything below, "orchestration" means the MutiS config (`mutis`).

Goal: DeepSeek (or any OpenAI-compatible model with tool calling) can WORK ON FILES inside AI Hub — a Rust port of
C:\TOOL\deepseek-worker\server.py (read it; same idea: chat completions + tools loop), hardened with the lessons below.

1. New provider kind `deepseek_agent` (src/providers/deepseek_agent.rs). Config fields: `base_url`, `api_key`, `models`
   like openai_compat (seed a provider "DeepSeek làm việc (Deploy)" with base_url https://api.deploy.com/api/v1, model
   deepseek-ai/deepseek-v4.1-flash-1017, empty key; Settings lets the user "Dùng key của Deploy (DeepSeek)" = copy the
   key server-side from the existing deploy-deepseek provider without sending it to the browser).
   Only usable when orchestration.enabled && deepseek_agent; otherwise the chat endpoint returns error
   "Bật Điều phối › DeepSeek làm việc trên file để dùng nguồn này".
2. Agent loop: POST {base_url}/chat/completions (non-streaming, with `tools`, max_tokens 8192, temperature 0.2), reuse the
   10a retry. Tools (all paths relative to the chat folder; reject absolute paths, `..`, symlink escapes, and anything under
   .git/, target/, node_modules/, .codegraph/):
   - list_dir(path) — max 300 entries;
   - read_file(path, start_line?, end_line?) — returns numbered lines, max 400 lines per call;
   - edit_file(path, old_text, new_text) — old_text must match exactly once (else error telling the count);
   - write_file(path, content) — create/overwrite, UTF-8;
   - NO command execution.
   Max steps = 15 (configurable 5–40 in orchestration as `deepseek_max_steps`). Stream progress as `log` events
   ("Đọc src/x.rs 1–120", "Sửa src/x.rs", ...) and the model's final text as `delta`. Token line in the same format as
   codex: "Token: vào X (cache 0), ra Y (suy luận 0)" summed over steps (from `usage`). Cancellation must stop the loop.
3. Lessons → safeguards:
   - Empty final answer / finish without any tool call while the task asked for edits → error event
     "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc".
   - Before reading a file longer than deepseek_max_file_lines without a line range, return a tool error telling the model
     to read by ranges; emit `log` "Cảnh báo: <file> dài N dòng".
   - "Edit-only mode": if the user message contains fenced blocks headed `FILE: <path>` (optionally `LINES a-b`), inject those
     file excerpts into the first prompt so the model can edit without reading.
   - ALWAYS produce a diff summary at the end, even after an error/cancel: snapshot the content of every file touched
     (before first write) and emit a `log` per file "Đã sửa <path> (+a −b dòng)" plus a final `diff` event with a unified
     diff (cap 20 KB). Add `diff` to the SSE event names the UI understands (render in a collapsed "Thay đổi" block).
4. Routing: the planner prompt (plans) must include the orchestration routing table (when enabled) and describe
   `deepseek_agent` as: "việc nhỏ 1–3 file, đã biết chính xác vị trí, theo mẫu có sẵn; không chạy lệnh". The reassign()
   function from step 8c must map kinds using the routing table when orchestration is enabled (fallback to the old rule).
   Add kind "small_edit" to the plan task kinds (UI select + validation).
5. Unit tests: path sandbox (reject `..`, absolute, .git), edit_file exact-once semantics, edit-only block parsing,
   diff summary counts, routing-based reassign.
Update plans/orchestration.md progress log. Reply per RULES.md.
