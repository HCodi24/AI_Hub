# Rules for every Codex step in AI Hub (C:\TOOL\ai-hub)

- Read SPEC.md, plans/orchestration.md and the existing code before changing anything. Keep existing features working —
  do NOT drop behaviour when rewriting a function (a previous rewrite silently removed buttons).
- Build/verify (all must pass): `CARGO_TARGET_DIR=target-codex cargo build --release --offline`,
  `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline`, `node --check ui/app.js`.
- ENCODING: write files as UTF-8 only (never PowerShell Set-Content/Out-File/redirection). After editing, scan src/ and
  ui/ for U+FFFD, `Ã`, `á»`, `áº` and Vietnamese words turned into `?` (e.g. `l?i`, `Ch?y`, `k? ho?ch`) and fix them.
- Readable code: no minified one-liners; small functions; Rust formatted with `cargo fmt`.
- Never run real AI calls (codex/claude/HTTP APIs) yourself; the reviewer tests live.
- Never print or log API keys.
- Reply: files changed, verification commands + results, and an explicit list of anything NOT done.
