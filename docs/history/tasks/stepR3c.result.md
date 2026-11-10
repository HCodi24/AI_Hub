## R3c complete

I reflowed the overlength lines in the UI files and marked R3 complete in [docs/design/refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md). No behavior change was intended.

Counts below use the gate’s character-counting rule. **Every target file now has zero lines over 200 characters.**

| File | Before | After |
|---|---:|---:|
| `ui/js/api.js` | 0 | 0 |
| `ui/js/chat.js` | 1 | 0 |
| `ui/js/dev.js` | 7 | 0 |
| `ui/js/extensions.js` | 6 | 0 |
| `ui/js/main.js` | 13 | 0 |
| `ui/js/mode-chip.js` | 3 | 0 |
| `ui/js/mutis-page.js` | 9 | 0 |
| `ui/js/plans.js` | 17 | 0 |
| `ui/js/settings.js` | 5 | 0 |
| `ui/js/state.js` | 0 | 0 |
| `ui/js/util.js` | 0 | 0 |
| `ui/index.html` | 24 | 0 |
| `ui/styles/base.css` | 0 | 0 |
| `ui/styles/chat.css` | 3 | 0 |
| `ui/styles/dev.css` | 0 | 0 |
| `ui/styles/extensions.css` | 0 | 0 |
| `ui/styles/layout.css` | 4 | 0 |
| `ui/styles/main.css` | 0 | 0 |
| `ui/styles/mutis.css` | 0 | 0 |
| `ui/styles/plans.css` | 1 | 0 |
| `ui/styles/settings.css` | 3 | 0 |

Each edited JavaScript file passed `node --check`.

Both requested gates passed with exit code 0, using a process-scoped PowerShell execution-policy bypass:

- `scripts/verify.ps1` — fmt, clippy, test, node, encoding, and line-length checks passed.
- `scripts/smoke.ps1` — passed.

I did not use or download a formatter. I also left the other working-tree entries untouched: `scripts/verify.ps1` and the untracked `docs/history/tasks/stepR3b.md`, `stepR3b.result.md`, and `stepR3c.md`.