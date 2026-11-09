Implemented Step 10d3 self-diagnosis and setting changes.

- Diagnosis now streams provider `log` events over SSE, reuses the chat provider runner, and sends key-free MutiS settings, 30-day stats, and up to 150 recent failure events. It resolves a connected lead, runs with read permission, and excludes the file-editing DeepSeek agent from diagnosis.
- Fewer than five events produce and save the local “Chưa đủ dữ liệu để chẩn đoán” result without calling a provider. Valid diagnosis JSON is shape-checked, unknown keys are dropped, results are saved to diagnosis history, and successful results are logged as `diagnosis` events.
- `/api/mutis/apply` now uses a single server-side allowlist function, validates values against MutiS bounds, and returns `{old, new}`. The UI shows live logs, health and finding cards, confirmation before applying eligible changes, and a history that can reopen past results.
- Updated [README.md](/C:/TOOL/ai-hub/README.md), [SPEC.md](/C:/TOOL/ai-hub/SPEC.md), and the progress log in [plans/mutis.md](/C:/TOOL/ai-hub/plans/mutis.md).

**Verification passed:** `cargo build --release --offline`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline` (37 passed), `node --check ui/app.js`, the requested UTF-8/mojibake scan, and `git diff --check`.

**Not done:** Live provider calls and an interactive browser flow were not exercised, in accordance with `.tasks/RULES.md`.