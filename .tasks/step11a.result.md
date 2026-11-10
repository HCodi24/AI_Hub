## Step 11a complete

Implemented dev mode with `--dev` and `AI_HUB_DEV=1`, repo discovery through `AI_HUB_REPO` or the running executable’s parent directories, and a clear startup error if the repository root cannot be found.

In dev mode, `/`, `/app.js`, and `/style.css` read from `ui\` on each request with `Cache-Control: no-store`. A one-second watcher sends UI and Rust changes over SSE. The UI shows a DEV badge, reloads on UI changes, and provides a rebuild banner with live build output and compiler error tail. Successful builds restart the process with exit code 75; rebuilds are refused while tracked AI jobs are active. Auto rebuild is available in **Settings › Tự phát triển** and defaults off. Release builds keep the embedded UI.

Added `dev.ps1` and `dev.cmd`, documented the mode in README, and updated the Step 11a progress log. `dev.ps1` has a UTF-8 BOM.

**Files changed:** `src/main.rs`, `src/server.rs`, `src/config.rs`, new `src/dev.rs`, `ui/app.js`, `ui/index.html`, `ui/style.css`, `dev.ps1`, `dev.cmd`, `README.md`, `plans/self-dev.md`.

**Verification:** offline release build passed; offline clippy with warnings denied passed; all 39 offline tests passed, including the new repo discovery and snapshot change tests; `node --check ui/app.js` passed; `git diff --check` passed; the requested source/UI encoding scan found no matches.

**Not done:** I did not launch the app interactively or exercise the rebuild/restart flow in a browser.