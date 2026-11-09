Step 7b — fix review findings in AI Hub (C:\TOOL\ai-hub). Build with
`CARGO_TARGET_DIR=target-codex cargo build --release --offline`, `cargo clippy --offline`, `cargo test --offline`; all clean.
Do not install or run CodeGraph.

Live test results from the reviewer:
1. MOJIBAKE: `GET /api/extensions` returns double-encoded Vietnamese, e.g. description
   "Láº­p chá»‰ má»¥c mÃ£ nguá»“n..." and label "CÃ i Ä‘áº·t". src/extensions.rs was saved with text that was
   UTF-8 decoded as Windows-1252 and re-encoded. Rewrite every Vietnamese string in src/extensions.rs as correct
   UTF-8 ("Lập chỉ mục mã nguồn để hỗ trợ công cụ AI hiểu dự án.", "Cài đặt", "Kết nối Codex", "Kết nối Claude",
   "Lập chỉ mục thư mục", "Nâng cấp", "Gỡ cài đặt", "<thư mục>/.codegraph/"). Write the file with a method that
   preserves UTF-8 (not PowerShell Set-Content default encoding). Then grep ALL files in src/ and ui/ for the
   mojibake markers `Ã`, `á»`, `áº`, `Ä‘`, `Æ°` and fix any other occurrence. Add a unit test asserting
   CATALOG strings contain no 'Ã' and that the install label == "Cài đặt".
2. Dedupe still not working: `/api/cli-discover/codex` still returns both
   `...\npm\codex.cmd` (source PATH, codex-cli 0.167.0) and
   `...\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`
   (source "npm bundled", same version). Rule: drop a candidate whose path lies inside
   `<dir of a .cmd candidate>\node_modules\` and has the same version as that .cmd. Add a unit test for this rule
   (pure function over a Vec of candidates).
3. src/extensions.rs is written as minified one-liners. Reformat it into normal readable Rust (run `cargo fmt`
   on the whole crate is fine).
4. extensions::status runs `codegraph --version` with blocking std::process::Command, no timeout and no
   CREATE_NO_WINDOW, inside an async handler. Reuse the existing async CLI helpers (resolve via the same
   discovery as cli_support incl. registry PATH, CREATE_NO_WINDOW, 10 s timeout, kill on timeout), and call it
   without blocking the tokio runtime.
Reply with files changed and verification.
