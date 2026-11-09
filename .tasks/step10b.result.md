Implemented Step 10b.

**What changed**

- Added `deepseek_agent`, with a Deploy seed provider, server-side Deploy key reuse, MutiS enablement checks, and a file-only tool loop. Its tools enforce path restrictions, bounded reads, exact-once edits, progress logs, token totals, cancellation, and a final diff event. The UI displays that diff in a collapsed **“Thay đổi”** block. See [deepseek_agent.rs](/C:/TOOL/ai-hub/src/providers/deepseek_agent.rs), [server.rs](/C:/TOOL/ai-hub/src/server.rs), and [app.js](/C:/TOOL/ai-hub/ui/app.js).
- Renamed the orchestration module and config types to MutiS, added the `orchestration` config alias and legacy endpoint aliases, and moved event history to the MutiS data directory with first-start migration. Existing configs also receive the seeded DeepSeek agent provider if missing. See [mutis.rs](/C:/TOOL/ai-hub/src/mutis.rs) and [config.rs](/C:/TOOL/ai-hub/src/config.rs).
- Updated MutiS settings labels, routing options, planner instructions, and `small_edit` validation. Added routing-based reassignment with the previous routing behavior as fallback. Updated [SPEC.md](/C:/TOOL/ai-hub/SPEC.md), [README.md](/C:/TOOL/ai-hub/README.md), and the progress log in [plans/mutis.md](/C:/TOOL/ai-hub/plans/mutis.md).
- Fixed Codex resume argv generation so each round builds a fresh vector through `codex_args`; stop-early phrases are checked only in the final paragraph, including CRLF messages. See [codex_cli.rs](/C:/TOOL/ai-hub/src/providers/codex_cli.rs).

**Verification**

- `CARGO_TARGET_DIR=target-codex cargo build --release --offline` — passed.
- `cargo clippy --offline --all-targets -- -D warnings` — passed.
- `cargo test --offline` — passed, 22 tests.
- `node --check ui/app.js` — passed.
- Encoding and MutiS UI label scan — no matches; `git diff --check` — clean.

**Not done**

- No live AI/API or CLI run was performed, as `.tasks/RULES.md` prohibits real AI calls. Live behavior still needs reviewer testing.
- Later MutiS steps remain outside this work.
- Preserved the pre-existing `.tasks/step10b.md` modification and the `.tasks/step10b2.md` file that appeared during the turn; I did not edit either.