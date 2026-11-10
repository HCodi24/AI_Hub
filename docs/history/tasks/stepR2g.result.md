## R2g complete

Moved the six business logic modules into `src/features/`, keeping the existing `crate::chat`-style paths available through root re-exports. Centralized CLI executable resolution, candidate discovery, version probing, deduplication, and safety checks in `src/providers/discovery.rs`; `cli_support.rs` now uses that resolver. Updated the README source map and marked R2 complete in the refactor doc.

The verification script now **fails** when Rust source lines exceed 200 characters and continues to report UI overlong lines as warnings. The UI warning remains: 102 lines exceed the limit.

## Gates

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.
- `scripts/verify.ps1` — passed: fmt, clippy, tests, Node syntax, encoding, and Rust line length.
- `scripts/smoke.ps1` — passed.
- `git diff --check` — passed.

PowerShell required process-scoped `ExecutionPolicy Bypass` to run the scripts.

## Final `src` tree

```text
src/
├── api/
│   ├── chat.rs (483)
│   ├── config.rs (45)
│   ├── dev.rs (583)
│   ├── extensions.rs (287)
│   ├── mod.rs (211)
│   ├── mutis.rs (607)
│   ├── plans.rs (502)
│   ├── providers.rs (608)
│   ├── static_ui.rs (56)
│   └── verify.rs (104)
├── app.rs (79)
├── core/
│   ├── config.rs (571)
│   ├── mod.rs (4)
│   ├── paths.rs (29)
│   ├── process.rs (47)
│   └── text.rs (73)
├── features/
│   ├── chat.rs (254)
│   ├── dev.rs (205)
│   ├── extensions.rs (251)
│   ├── mod.rs (6)
│   ├── mutis.rs (360)
│   ├── plans.rs (342)
│   └── verify.rs (626)
├── main.rs (100)
└── providers/
    ├── anthropic.rs (141)
    ├── claude_cli.rs (156)
    ├── cli_support.rs (471)
    ├── codex_cli.rs (431)
    ├── deepseek_agent.rs (601)
    ├── discovery.rs (591)
    ├── mod.rs (210)
    └── openai_compat.rs (166)
```

**Total:** 9,200 Rust source lines. Nothing remains undone for R2g.