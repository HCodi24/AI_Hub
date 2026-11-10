Step D1 of AI Hub — merge "DeepSeek làm việc (Deploy)" into the "Deploy (DeepSeek)" source. FIRST read docs/CODEX_RULES.md and
docs/SPEC.md. Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1 (smoke only kills the PID it
started — run it). No real AI calls. The user runs the app in dev mode from this repo: never stop ai-hub processes.

Problem (user): there are two sources for the same Deploy account — "Deploy (DeepSeek)" (chat) and "DeepSeek làm việc (Deploy)"
(kind `deepseek_agent`, reads/edits files). Key/URL must be copied, connection is tested twice (Deploy shows ✓, the agent one
stays grey → "chưa kết nối — sẽ dùng Codex CLI"). They want ONE source; file work is a mode of it.

Design:
1. Provider config: add `agent_capable: bool` (serde default false) for kind `openai_compat`. Meaning: "AI may use file tools
   (list/read/edit/write) inside the chat folder through this source". UI: Settings › provider panel gets a checkbox
   "Cho phép AI đọc/sửa file trong thư mục chat (chế độ agent)" for openai_compat providers (tooltip: không chạy lệnh).
2. Chat request: add optional `file_tools: "none"|"read"|"write"` (default "none"). Server /api/chat: if provider.kind ==
   "openai_compat" && provider.agent_capable && file_tools != "none" → run the existing DeepSeek agent loop
   (src/providers/deepseek_agent.rs) using THIS provider's base_url/api_key/model, folder = chat folder (required, validated as
   today). Tool set: "read" → list_dir + read_file only; "write" → all four (edit_file/write_file allowed). Keep every existing
   safeguard (path sandbox, no commands, diff event, step limit, long-file warning, retry, cancel, token line, status/heartbeat).
   If file_tools != none but the provider is not agent_capable → 400 "Bật «Cho phép AI đọc/sửa file» cho nguồn này trong Cài đặt".
   Remove the old gate "Bật MutiS › DeepSeek làm việc trên file" from plain chat (MutiS flag `deepseek_agent` now only controls
   whether MutiS may route small tasks to the agent mode; keep that meaning in plans/reassign).
3. UI composer: when the selected source is agent_capable show a select "Công cụ file": "Không (chỉ chat)" / "Chỉ đọc file" /
   "Được sửa file" (default Không; stored per chat like `permission`; sent as `file_tools`). Hide it for other sources. In the
   mode chip tooltip/summary show it when not "none". Chat/plan runs started by MutiS/Chia việc for the small role map the task's
   permission to file_tools (write→write, read→read) automatically.
4. MutiS: role `small` and routing kind `small_edit/docs` must reference the real provider id (e.g. `deploy-deepseek`) — the
   assignee for an agent-capable openai_compat source. Replace the special-casing of kind "deepseek_agent" in mutis.rs/plans.rs/
   providers.rs/config.rs: a provider is "small-capable" when kind == openai_compat && agent_capable. Fallback order for small:
   agent-capable provider → codex_cli → claude_cli. Planner prompt text: describe it as "<name> (chế độ agent: đọc/sửa file, không
   chạy lệnh)". Keep the internal Provider implementation file deepseek_agent.rs (rename the struct if you like) but it must no
   longer be a user-visible source kind.
5. Migration on config load (must be idempotent and tested): if a provider of kind `deepseek_agent` exists:
   - find an openai_compat provider with the same base_url (prefer id `deploy-deepseek`); if found: set its `agent_capable = true`
     (and add any models the agent had that the parent lacks), remap every reference (mutis.roles.*.provider_id, routing, planner
     ids, saved plans are NOT rewritten but must still resolve: treat unknown provider ids that equal the removed id as the parent
     at run time), then DELETE the deepseek_agent provider;
   - if no parent found: convert it in place to kind `openai_compat` with `agent_capable = true`.
   Fresh seed: no deepseek-agent-deploy; `deploy-deepseek` is seeded with `agent_capable = false`.
   Keep accepting kind "deepseek_agent" in validation only for loading old files (never offer it in the UI/provider kinds list).
6. Provider list UI: remove the "Dùng key của Deploy" button/endpoint and any separate pill/dropdown entry for the agent.
   Connection status is the source's own (one dot).
7. Unit tests: migration (parent found / not found / idempotent / roles remapped), file_tools→tool-set mapping, the 400 case,
   small-capable fallback selection, config round-trip with agent_capable.
Update README (Nguồn AI) and docs/SPEC.md. Reply per docs/CODEX_RULES.md and list anything not done.
