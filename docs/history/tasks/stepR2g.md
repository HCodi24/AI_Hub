Step R2g of AI Hub refactor (last backend step). FIRST read docs/CODEX_RULES.md and docs/design/refactor.md.
PURE MOVE/FORMAT, no behaviour change. Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1.

1. Create src/features/ and move the business-logic modules there (keep file contents, fix paths):
   src/chat.rs → features/chat.rs, plans.rs → features/plans.rs, mutis.rs → features/mutis.rs,
   verify.rs → features/verify.rs, extensions.rs → features/extensions.rs, dev.rs → features/dev.rs,
   with src/features/mod.rs. Keep `crate::chat` etc. working ONLY if trivial via re-exports; otherwise update call sites.
2. CLI discovery: move the non-HTTP discovery code (candidate dirs, registry PATH, version probing, dedupe, safety rule
   for Claude desktop) out of src/api/providers.rs into src/providers/discovery.rs; api/providers.rs keeps only handlers.
   cli_support.rs uses discovery.rs for resolution (one implementation only).
3. Long lines: make every line in src/**/*.rs ≤ 200 chars (rustfmt can't split long string literals / json! macros —
   split them manually, e.g. `concat!` or builder lines). Then change scripts/verify.ps1 so that line length > 200 in
   src/**/*.rs is an ERROR (ui/ stays a warning until R3).
4. Update README "Cấu trúc mã nguồn" table and docs/design/refactor.md (mark R2 done).
Report final src tree with line counts and gate results.
