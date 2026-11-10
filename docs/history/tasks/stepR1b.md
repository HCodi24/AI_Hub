Step R1b of AI Hub refactor. FIRST read docs/CODEX_RULES.md and docs/design/refactor.md.
Files were just moved (R1a): docs/SPEC.md, docs/design/*.md, docs/CODEX_RULES.md, docs/history/tasks/*, scripts/dev.ps1,
scripts/dev.cmd. Build with CARGO_TARGET_DIR=target/codex. Do NOT change app behaviour beyond the items below.

1. `scripts/verify.ps1` (UTF-8 with BOM; set [Console]::OutputEncoding UTF8): one command that runs, from the repo root,
   with CARGO_TARGET_DIR=target\verify unless already set: `cargo fmt --check`, `cargo clippy --offline --all-targets
   -- -D warnings`, `cargo test --offline`, `node --check` on every ui/**/*.js, a mojibake/UTF-8 scan of src/ ui/ scripts/
   (U+FFFD, "Ã", "á»", "áº", invalid UTF-8; Vietnamese word "?" pattern inside string literals), and a check that no
   line in ui/**/*.js|css|html and src/**/*.rs is longer than 200 chars (REPORT ONLY as a warning for now — R2/R3 fix them).
   Print ~1 line per check: "✓ fmt", "✗ clippy (xem log: target\verify\logs\clippy.txt)", final "ĐẠT"/"LỖI"; exit 0/1.
   Full outputs go to target\verify\logs\*.txt.
2. `scripts/smoke.ps1`: builds (debug, CARGO_TARGET_DIR=target\smoke), copies a fresh isolated config dir under
   target\smoke\data (seeded by the app itself — do NOT copy the user's real config), starts the app with
   AI_HUB_CONFIG_PATH=<that>\config.json on port 7911 --no-open, then:
   - GETs every GET route that needs no body/params (enumerate them from the router; keep a list in the script) and fails
     on any 5xx or connection error; also GET / , /app.js, /style.css must be 200;
   - POST /api/providers/codex-cli/test and /api/mutis (PUT with the current value) must not 5xx;
   - stops the app (kill only the PID it started) and prints "SMOKE: ĐẠT"/"SMOKE: LỖI" + failures. Must not call any AI.
3. Defaults: `verify_ignore` default becomes ["docs/**", "**/*.md"] (config.rs default + ui/app.js fallback). Keep
   existing user configs unchanged (serde default only applies when missing).
4. MutiS verify auto-detect: add `scripts/verify.ps1` (run with `powershell -NoProfile -ExecutionPolicy Bypass -File`) as
   the FIRST candidate before scripts/verify/verify.sh → Cargo → npm. Update the auto-detect unit test.
5. dev mode: the Rust watcher must ignore docs/ and scripts/ (only src/**, Cargo.toml, Cargo.lock); verify it still does.
   Self-dev "Chạy test" keeps CARGO_TARGET_DIR=target\dev-check.
6. README: in "Cấu trúc mã nguồn" and "Chế độ dev", mention scripts/verify.ps1 and scripts/smoke.ps1.
Run scripts/verify.ps1 and scripts/smoke.ps1 yourself at the end (smoke starts a local server — allowed; no AI calls).
Update docs/design/refactor.md progress. Reply per CODEX_RULES.md.
