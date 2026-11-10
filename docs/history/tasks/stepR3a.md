Step R3a of AI Hub refactor (frontend, part 1). FIRST read docs/CODEX_RULES.md and docs/design/refactor.md.
PURE REFACTOR: the UI must look and behave exactly the same. Build with CARGO_TARGET_DIR=target/codex.
Gate: scripts/verify.ps1 + scripts/smoke.ps1 must pass (update smoke.ps1 to also GET every new UI file).

1. Static serving (src/api/static_ui.rs): serve any file under `ui/` by path for `/` (index.html), `/js/*.js`,
   `/styles/*.css` (and keep `/app.js`, `/style.css` working until the split is done, then remove them).
   - Release/non-dev: files are embedded. Use a build.rs that walks `ui/` and generates `$OUT_DIR/ui_assets.rs` with a
     `static UI_ASSETS: &[(&str, &[u8])]` table (`include_bytes!` of each absolute path) + `cargo:rerun-if-changed=ui`.
     No new crates (offline). Content-Type by extension (html, js → text/javascript, css, svg, png).
   - Dev mode: read from `<repo>/ui/<path>` on each request (no-store), reject `..`/absolute paths.
   - The dev watcher must watch `ui/**` recursively (it currently polls `ui\*`).
2. Split ui/app.js (1,778 lines) into ES modules under ui/js/, loaded by `<script type="module" src="/js/main.js">`:
   suggested files — util.js (el, $, escaping, toast, formatting), api.js (fetch helpers, readSse), state.js (the shared
   `state` object + config cache), chat.js (messages, composer, streaming), mode-chip.js (mode bar + MutiS team
   popover), settings.js (settings dialog, providers, connection status, CLI discovery, MutiS settings),
   plans.js (Chia việc), mutis-page.js (stats + self-diagnosis), extensions.js (Tiện ích), dev.js (dev badge, live
   reload, rebuild banner, Tự phát triển page, Chat phát triển), main.js (startup + wiring all event listeners).
   Move code unchanged; convert globals to explicit imports/exports; no circular-import surprises at startup
   (functions referenced only inside handlers are fine).
3. Split ui/style.css into ui/styles/{base,layout,chat,settings,plans,mutis,extensions,dev}.css imported from
   ui/styles/main.css via @import (served as /styles/main.css), or several <link> tags — your choice, same rendering.
4. Delete ui/app.js and ui/style.css when nothing references them. `node --check` every ui/js/*.js file
   (verify.ps1 already loops ui/**/*.js — confirm).
5. Update README "Cấu trúc mã nguồn" and docs/design/refactor.md log.
Report: new ui tree with line counts, gate results, anything not done.
