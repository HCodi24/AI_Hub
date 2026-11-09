Implemented the 10c changes and updated the progress log in [plans/mutis.md](C:/TOOL/ai-hub/plans/mutis.md).

- Fixed the mode popover outside-click handler to ignore detached targets. Selecting MutiS keeps the popover open and scrolls “Đội MutiS” into view.
- Added the `/api/verify` SSE endpoint and [verify.rs](C:/TOOL/ai-hub/src/verify.rs) with custom command and project detection, MOJIBAKE checks, a 15-minute total timeout, ANSI stripping, and process-tree termination on cancellation.
- Added persisted verification and review results to plan tasks. The runner now supports lead-role review with connected-provider fallback, review and fix-task event logging, automatic fix rounds, per-plan verification, per-task review controls, and optional sequential continuation.
- Added unit coverage for detection order, mojibake cases, JSON extraction, and fix-round limits.

**Verification passed:** release offline build, clippy with `-D warnings`, 30 unit tests, `node --check ui/app.js`, encoding scan, and `git diff --check`.

**Not done:** I did not run live AI/API calls or exercise the full flow in the running app, as `.tasks/RULES.md` prohibits real AI calls and reserves live reviewer testing. The workspace already had changes in `.tasks/step10c.md`; I left that file as found.