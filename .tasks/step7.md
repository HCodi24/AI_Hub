Step 7 of AI Hub (C:\TOOL\ai-hub). Read SPEC.md and the current code (steps 5–6 are done).
Build with `CARGO_TARGET_DIR=target-codex cargo build --release --offline` and
`CARGO_TARGET_DIR=target-codex cargo clippy --offline`; both clean. Do NOT actually install anything or run
any extension command yourself — only implement and unit-test the code.

Feature: "Tiện ích" — a place in the app to manage extensions/utilities for the AI tools
(first one: CodeGraph, https://github.com/colbymchenry/codegraph).

UI
1. Left sidebar: add a "Tiện ích" button (below "Cài đặt"). It opens a full-width page/view (not a small dialog)
   with a grid of extension cards. Each card: name, one-line description, homepage link (opens in new tab,
   rel=noopener), status badges, and action buttons. A "Nhật ký" panel at the bottom streams command output.
2. Status badges per card (computed by backend, refreshed when the page opens and after each action):
   - "Chưa cài" / "Đã cài · <version>"
   - "Codex: đã kết nối / chưa" and "Claude: đã kết nối / chưa" (only if the extension supports them)
   - "Thư mục hiện tại: đã lập chỉ mục / chưa" (uses the folder of the active chat; hide if none)
3. Action buttons appear only when they make sense (e.g. "Cài đặt" only if not installed).
   EVERY action first shows a confirm dialog with the exact command(s) that will run and which files may be
   modified, then runs and streams output to "Nhật ký". On finish: toast ✓/✗ and refresh status.

Backend
4. A built-in catalog (Rust, `src/extensions.rs`) of extension definitions. Commands are FIXED templates in the
   catalog — the browser only sends `{extension_id, action_id, folder?}`; it can never send command text.
   CodeGraph definition:
   - check installed: `codegraph --version` (resolve executable like cli_support: PATH, registry PATH,
     %APPDATA%\npm, `%USERPROFILE%\.codegraph\bin`, `%LOCALAPPDATA%\codegraph\bin`)
   - actions:
     * install:      `npm i -g @colbymchenry/codegraph`   (needs npm; resolve npm.cmd the same way)
     * connect_codex: `codegraph install --target=codex --yes`   modifies `~/.codex/config.toml`
     * connect_claude: `codegraph install --target=claude --yes` (if that target name is wrong for the
        installed version the command will fail and the log shows why — that is acceptable)  modifies `~/.claude.json`
     * index_folder: `codegraph init` with cwd = folder (folder must exist; reuse the %/" path checks)
     * upgrade:      `codegraph upgrade`
     * uninstall:    `npm rm -g @colbymchenry/codegraph`
   - status checks: installed+version; Codex connected = `~/.codex/config.toml` contains a `codegraph` mcp
     server entry (parse TOML with the `toml` crate if cached offline, else a substring check for
     `[mcp_servers.codegraph`); Claude connected = `~/.claude.json` has `mcpServers.codegraph`;
     folder indexed = `<folder>/.codegraph/codegraph.db` exists.
5. Before any action that modifies a config file, copy that file to
   `<config_dir>/ai-hub/backups/<filename>.<YYYYMMDD-HHMMSS>.bak` and print the backup path in the log.
6. Endpoints: `GET /api/extensions?folder=...` (catalog + status), `POST /api/extensions/{id}/actions/{action}`
   (body `{folder?}`) returning SSE `log` / `error` / `done {ok}` events. Only one action at a time per
   extension (409 if busy). Stop button cancels (kill process tree like cli_support). Timeout 10 minutes.
   Run with CREATE_NO_WINDOW; `.cmd` via the existing safe cmd wrapper; strip ANSI color codes from output.
7. Design the catalog so adding a new extension is just adding one more definition (name, description,
   homepage, executable name, version args, actions, status checks).

Also update README.md ("Tiện ích" section) and SPEC.md (new section). Reply with files changed and verification.
