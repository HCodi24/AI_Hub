Step 4 of AI Hub (C:\TOOL\ai-hub). Build with `cargo build --release --offline` and `cargo clippy --offline`; both clean.
Do not run codex/claude yourself.

Bug: when ai-hub.exe is launched from a terminal whose inherited PATH lacks `%APPDATA%\npm`, the app says
"Chưa cài Codex CLI" although `C:\Users\<user>\AppData\Roaming\npm\codex.cmd` exists. Make CLI discovery robust
in `src/providers/cli_support.rs` (`resolve_executable`), used by both chat and /api/cli-status:

1. Optional explicit path: add an optional `executable` field to ProviderConfig (serde default, empty = auto).
   If set and the file exists, use it. Expose it in the Settings dialog for codex_cli / claude_cli providers as
   "Đường dẫn file chạy (để trống = tự tìm)". Keep the "empty keeps existing" logic only for api_key.
2. Otherwise search, in order, for `<name>.exe`, `<name>.cmd`, `<name>`:
   a. the process PATH;
   b. on Windows, the CURRENT user and machine PATH read fresh from the registry
      (HKCU\Environment\Path and HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment\Path),
      expanding %VAR% references (REG_EXPAND_SZ); use the `winreg` crate only if it is already in the offline
      cargo cache, otherwise run `reg query` via std::process with CREATE_NO_WINDOW and parse the output;
   c. well-known install dirs: `%APPDATA%\npm`, `%LOCALAPPDATA%\Programs\nodejs`, `%USERPROFILE%\.local\bin`,
      and for codex also `%LOCALAPPDATA%\OpenAI\Codex\bin\*\codex.exe` (newest modified first).
   Skip PATH entries that are empty or contain unexpanded `$`/`%` text.
3. When a `.cmd` shim is used, the child also needs node on PATH: prepend the shim's directory and the
   directory of `node.exe` (if found by the same search) to the child's PATH env var.
4. /api/cli-status should also return the resolved `path` so the Settings dialog can show where it was found.
5. The "not installed" error should list the places that were searched (short).

Reply with files changed and verification.
