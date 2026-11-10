Implemented the 10c2 fixes.

- Added cached server-side role resolution at `GET /api/mutis/roles/resolve`. The UI calls it before planning, task runs, and reviews, and uses the connected lead fallback for review.
- When no reviewer is connected, review is saved as `SKIPPED` with the requested note, the task is marked complete unless verification failed, and no fix round is used. The plan chip displays “Review: bỏ qua.”
- High or medium severity review failures create and run a “Sửa lỗi review: …” task within the configured round limit.
- Verification now scans changed and untracked files, caps the scan at 500 files, skips files over 1 MB and binary files, and reports invalid UTF-8 separately. The editable `verify_ignore` globs apply to MOJIBAKE scanning only.
- Added state transition, invalid UTF-8, untracked file, and ignore glob tests. Updated the progress log in [plans/mutis.md](C:/TOOL/ai-hub/plans/mutis.md).

Changed files: [config.rs](C:/TOOL/ai-hub/src/config.rs), [server.rs](C:/TOOL/ai-hub/src/server.rs), [verify.rs](C:/TOOL/ai-hub/src/verify.rs), [app.js](C:/TOOL/ai-hub/ui/app.js), [index.html](C:/TOOL/ai-hub/ui/index.html), and [mutis.md](C:/TOOL/ai-hub/plans/mutis.md).

**Verification:** `cargo fmt --check`, offline release build, clippy with `-D warnings`, offline tests (33 passed), `node --check ui/app.js`, `git diff --check`, and the configured encoding scan passed.

**Not done:** I did not make live AI calls or exercise the browser flow; `.tasks/RULES.md` prohibits real AI calls.