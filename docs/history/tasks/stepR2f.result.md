## R2f complete

Moved the plan handlers, planner prompt and SSE flow, reassignment logic, plan events, helpers, and related tests to [src/api/plans.rs](/C:/TOOL/ai-hub/src/api/plans.rs). The router and Host/Origin middleware now live in [src/api/mod.rs](/C:/TOOL/ai-hub/src/api/mod.rs), with routes grouped by feature. The middleware still checks the actual bound port, now carried in `AppState`.

Deleted `src/server.rs`, removed `mod server`, updated the README source table and refactor log, and confirmed there are no remaining `crate::server` references. `git diff --check` passed.

## Verification

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.
- `scripts/verify.ps1` — passed: fmt, clippy, tests, Node syntax, and encoding checks. Line-length check was report-only.
- `scripts/smoke.ps1` — passed.

## Final Rust line counts

| File | Lines | File | Lines |
|---|---:|---|---:|
| `src/api/chat.rs` | 434 | `src/api/config.rs` | 42 |
| `src/api/dev.rs` | 491 | `src/api/extensions.rs` | 190 |
| `src/api/mod.rs` | 204 | `src/api/mutis.rs` | 520 |
| `src/api/plans.rs` | 467 | `src/api/providers.rs` | 936 |
| `src/api/static_ui.rs` | 52 | `src/api/verify.rs` | 86 |
| `src/app.rs` | 75 | `src/chat.rs` | 233 |
| `src/core/config.rs` | 545 | `src/core/mod.rs` | 4 |
| `src/core/paths.rs` | 23 | `src/core/process.rs` | 43 |
| `src/core/text.rs` | 68 | `src/dev.rs` | 190 |
| `src/extensions.rs` | 236 | `src/main.rs` | 99 |
| `src/mutis.rs` | 351 | `src/plans.rs` | 332 |
| `src/providers/anthropic.rs` | 137 | `src/providers/claude_cli.rs` | 152 |
| `src/providers/cli_support.rs` | 664 | `src/providers/codex_cli.rs` | 422 |
| `src/providers/deepseek_agent.rs` | 542 | `src/providers/mod.rs` | 195 |
| `src/providers/openai_compat.rs` | 162 | `src/verify.rs` | 621 |

**Total: 8,516 lines across 30 Rust files.** No requested work remains undone.