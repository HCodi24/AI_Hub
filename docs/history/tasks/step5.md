Step 5 of AI Hub (C:\TOOL\ai-hub). Read SPEC.md and the code first.
Build with `CARGO_TARGET_DIR=target-codex cargo build --release --offline` and
`CARGO_TARGET_DIR=target-codex cargo clippy --offline` (the normal target/ exe may be locked by a running app).
Both must be clean. Do not run codex/claude yourself.

Feature: a compact "mode bar" next to the chat input showing which AI mode is active, styled like the
Claude desktop picker: model name in normal text, effort level in muted text, e.g.  `GPT-6 Luna  Max`
or `Deploy · DeepSeek V4.1 Flash  Mặc định`.

UI (ui/index.html, app.js, style.css):
1. In `.composer-footer` (ui/index.html ~line 57), on the left, add a button `#mode-chip`:
   [provider short name ·] model display name + muted effort label + small chevron. Keep the
   "Enter để gửi · Shift+Enter xuống dòng" hint but move it to the right side next to the send actions
   (hide it under 640px width).
2. Clicking the chip opens a small popover ABOVE the chip (closes on Esc / outside click), with:
   - "Nguồn": list of providers (radio-style rows, current one checked),
   - "Model": list of the selected provider's models (rows) + a free-text row "Model khác…",
   - "Mức suy luận": segmented buttons, shown only when the provider kind supports it (see below).
   Choosing items updates the existing top-bar selects/inputs (keep them; they stay in sync both ways)
   and the chip text immediately. Keyboard accessible (arrow keys/Tab, Enter to choose, aria-expanded).
3. Model display name: a helper `prettyModel(id)`: take the part after the last "/", drop a trailing
   date-like suffix (`-\d{4}$` or `-\d{8}$`), split on "-" / "_", capitalize words, uppercase known
   acronyms (gpt, ai, api), keep version tokens like "v4.1" -> "V4.1", "6" stays "6"; join "gpt"+"6"
   as "GPT-6". Examples: `gpt-6-luna` -> `GPT-6 Luna`, `deepseek-ai/deepseek-v4.1-flash-1017` ->
   `Deepseek V4.1 Flash`, `claude-opus-5-5` -> `Claude Opus 5.5` (consecutive single-digit tokens after
   a word join with "."), `opus` -> `Opus`. Show the raw id in the chip's `title` tooltip.
4. Effort label shown in Vietnamese: default -> "Mặc định", low -> "Thấp", medium -> "Vừa",
   high -> "Cao", xhigh -> "Rất cao", max -> "Max". If the provider does not support effort, show
   nothing after the model name.
5. Persist the selected effort per chat (saved with the chat like `permission`) and remember the last
   used provider/model/effort for new chats (localStorage, wrapped in try/catch).

Backend:
6. Add optional `effort` to the chat request and chat record: one of
   "default" | "low" | "medium" | "high" | "xhigh" | "max" (validate; reject anything else with 400).
7. Apply it per provider kind:
   - codex_cli: if not "default", pass `-c model_reasoning_effort="<effort>"` (works for exec and resume;
     place it with the other options before positional args). Supported: low, medium, high, xhigh, max.
   - openai_compat: if not "default", add `"reasoning_effort": "<effort>"` to the request body, but map
     xhigh/max -> "high" (OpenAI-style APIs accept low/medium/high). If the API returns HTTP 400 and the
     body mentions reasoning_effort, retry ONCE without the field and emit a `log`
     "Model không hỗ trợ mức suy luận, đã dùng mặc định".
   - anthropic: if not "default", enable extended thinking: `"thinking": {"type":"enabled","budget_tokens": N}`
     with N = low 2048, medium 8192, high 16000, xhigh/max 32000, and set max_tokens to N + 8192.
     Forward `thinking_delta` events as `reasoning`.
   - claude_cli: effort not supported -> ignore (UI hides the selector for this kind).
   Expose which kinds support effort to the UI via a constant in app.js (codex_cli, openai_compat, anthropic).
8. Default effort for new chats: "default".

Update README.md (short section "Thanh chế độ"). Reply with files changed and verification done.
