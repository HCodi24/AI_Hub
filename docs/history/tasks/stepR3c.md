Step R3c of AI Hub refactor. FIRST read docs/CODEX_RULES.md. PURE FORMAT, no behaviour change.
There is NO formatter installed and none can be downloaded — DO THE REFORMATTING BY HAND with your edit tools.
Gate: scripts/verify.ps1 (now fails ONLY on UI line length) + scripts/smoke.ps1 must both pass at the end.

Fix every line longer than 200 characters (count with: awk 'length>200' on each file — note awk counts bytes; the
gate counts characters, so lines just over 200 bytes with Vietnamese text may already be fine) in:
ui/js/*.js (60 lines), ui/index.html (26 lines), ui/styles/*.css (11 lines).
How:
- JS: one statement per line; break long object literals, argument lists, ternaries and template literals over
  several lines; split long `el(...)` trees into nested indented calls; long string concatenations into joined arrays.
- HTML: put each attribute group / child element on its own indented line; long inline text stays but wrapped tags.
- CSS: one declaration per line inside long rules.
Behaviour, DOM structure, ids, classes and texts must stay identical. Work file by file; after each JS file run
`node --check` on it. Finally run the two gates and report before/after counts per file.
Update docs/design/refactor.md log (R3 done) when the gate passes.
