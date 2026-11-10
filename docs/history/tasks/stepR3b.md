Step R3b of AI Hub refactor. FIRST read docs/CODEX_RULES.md and docs/design/refactor.md. PURE FORMAT, no behaviour change.
Gate: scripts/verify.ps1 + scripts/smoke.ps1.

1. Reformat every line longer than 200 characters in ui/index.html, ui/js/*.js, ui/styles/*.css into readable multi-line
   code (one statement per line, objects/arrays and long template literals broken across lines, HTML attributes and
   nested elements on separate lines, one CSS declaration per line for long rules). Keep identical behaviour and DOM.
2. Then make scripts/verify.ps1 treat UI lines > 200 chars as an ERROR (same as Rust).
3. Update docs/design/refactor.md log (R3 done).
Report how many long lines remained before/after and the gate results.
