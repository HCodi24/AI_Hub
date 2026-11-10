Implemented R3a as a frontend refactor.

- Added build-time embedded UI assets and path-based serving for `/` and files under `ui/`. Dev requests read from disk with `no-store`; paths containing traversal or absolute components are rejected.
- Split the UI into ES modules and CSS files, and updated `index.html` to load `/js/main.js` and `/styles/main.css`. Removed `ui/app.js` and `ui/style.css` and their routes.
- Updated smoke coverage to GET `/` and every supported UI asset. The dev watcher already traversed `ui/` recursively, so no watcher change was needed.
- Updated the README source map and the refactor log.

**New UI tree and line counts**

```text
ui/index.html                 160
ui/js/api.js                   49
ui/js/chat.js                 414
ui/js/dev.js                   48
ui/js/extensions.js            25
ui/js/main.js                 157
ui/js/mode-chip.js            239
ui/js/mutis-page.js            74
ui/js/plans.js                340
ui/js/settings.js             365
ui/js/state.js                 17
ui/js/util.js                  17
ui/styles/base.css             21
ui/styles/chat.css             40
ui/styles/dev.css              12
ui/styles/extensions.css       14
ui/styles/layout.css          107
ui/styles/main.css              8
ui/styles/mutis.css            21
ui/styles/plans.css            15
ui/styles/settings.css         30
```

**Gates**

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — passed.
- `scripts/verify.ps1` — passed, including Rust checks, nested JavaScript syntax checks, and encoding scan.
- `scripts/smoke.ps1` — passed, including GETs for every current UI asset.

The verify script reports 96 UI lines over 200 characters; that check is report-only. Nothing in the requested scope remains undone. The untracked `docs/history/tasks/stepR3a.md` is visible in the workspace status and was left untouched.