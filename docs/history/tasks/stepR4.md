Step R4 of AI Hub refactor (final). FIRST read docs/CODEX_RULES.md and docs/design/refactor.md.
Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1 (update smoke's route list).

1. Remove the legacy "orchestration" leftovers: the `/api/orchestration*` route aliases and any UI references
   (data-orch attributes may be renamed to data-mutis), the first-start migration from `<data>/orchestration/` stays
   (it is harmless) but must not fail if the folder is missing. KEEP `#[serde(alias = "orchestration")]` on the config
   field so old config.json files still load.
2. User-facing error messages in src/api/** and src/features/** must be Vietnamese and consistent
   (e.g. "Provider not found" → "Không tìm thấy nguồn AI", "Working folder does not exist" → "Thư mục làm việc không tồn
   tại", "Path is not allowed" → "Không được phép đổi thiết lập này", "Value out of range" → "Giá trị ngoài giới hạn",
   "Login is only available for Claude Code CLI" → "Chỉ Claude Code CLI mới đăng nhập được"). Grep for ApiError::new
   with English text and translate all. Log/internal anyhow contexts may stay English.
3. docs/SPEC.md: rewrite to describe the CURRENT architecture (src/api, features, core, providers; ui/js modules;
   build.rs embedding; scripts; data dirs; security model; all API routes grouped by feature). Remove outdated step notes.
4. README.md: refresh "Cấu trúc mã nguồn" (tree with one line per folder), "Chế độ dev" (scripts\dev.ps1), add a short
   "Kiểm tra" section (scripts\verify.ps1, scripts\smoke.ps1).
5. docs/design/refactor.md: mark R4 and the whole refactor done with a short summary (before/after line counts).
Report gate results and anything not done.
