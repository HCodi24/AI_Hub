Implemented Step 10b3.

- The MutiS popover now treats unknown provider status as “Đang kiểm tra kết nối…”, checks providers when the popover opens and when MutiS is selected at page load, and stays open while scrolling to “Đội MutiS”.
- Fallback selection follows the requested order and uses only providers confirmed connected. MutiS planning chooses a connected lead provider at runtime; if none is available, sending is blocked with a clear message.
- Migration now adds the seeded DeepSeek agent before assigning roles, uses its first model for “Làm việc nhỏ”, and fills empty role provider IDs when a provider is available.
- Updated the progress log in [plans/mutis.md](C:/TOOL/ai-hub/plans/mutis.md). Implementation changes are in [config.rs](C:/TOOL/ai-hub/src/config.rs), [server.rs](C:/TOOL/ai-hub/src/server.rs), and [app.js](C:/TOOL/ai-hub/ui/app.js).

**Verification passed:** `CARGO_TARGET_DIR=target-codex cargo build --release --offline`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline` (26 passed), `node --check ui/app.js`, and the UTF-8/mojibake scan.

**Not done:** Live browser testing and live AI/API calls; the project rules prohibit making those calls. No commit was created.