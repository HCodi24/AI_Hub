Step L1 of AI Hub — Claude Code CLI login through the WEB (not the Claude desktop app). FIRST read docs/CODEX_RULES.md.
Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1. Never run a real login yourself.

Facts verified by the reviewer on this machine (claude.exe 2.1.317, env CLAUDE*/ANTHROPIC_* removed, stdin/stdout piped,
no window, env BROWSER set to a no-op): `claude auth login --claudeai` prints to stdout:
  Opening browser to sign in…
  If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=...&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&...&state=...
  Paste code here if prompted >
then waits on stdin for the code that platform.claude.com shows after the user signs in on the web.
Problem today: the "Đăng nhập Claude" button opens a console running `claude auth login`, the CLI auto-opens the link and on
this machine the Claude desktop app grabs it. The user wants to sign in ON THE WEBSITE.

Implement (replace the current login endpoint behaviour):
1. `POST /api/providers/{id}/login/start` (claude_cli only): spawn the resolved claude.exe with args
   `auth login --claudeai` — piped stdin/stdout/stderr, CREATE_NO_WINDOW, env CLAUDE*/ANTHROPIC_* removed (existing
   helper) and `BROWSER` set to a harmless no-op (Windows: `cmd /c rem`) so the CLI does NOT open anything itself.
   Read stdout until the first `https://` URL (timeout 20 s) and return `{session_id, url}`. Keep the child in AppState
   (one login session at a time; a new start kills the previous one; kill after 10 min idle; kill on app exit).
   Validate the URL host is claude.com / claude.ai / anthropic.com before returning it (else error, kill child).
2. `POST /api/providers/{id}/login/code` {session_id, code}: write `code.trim()` + "\n" to the child's stdin; wait up to
   30 s for exit; then run the existing `claude auth status` check and return {ok, logged_in, message, output_tail}.
   Reject codes longer than 512 chars or containing control characters. Never log the code.
3. `POST /api/providers/{id}/login/cancel`: kill the child.
4. UI (Settings › Claude Code CLI, where "Đăng nhập Claude" is today):
   - Click → spinner → a panel:
     "1. Bấm **Mở trang đăng nhập** (mở trên trình duyệt, không dùng app Claude) → đăng nhập tài khoản Claude.
      2. Trang sẽ hiện một mã → dán vào đây → **Xác nhận**."
   - "Mở trang đăng nhập ↗" = `<a href=url target="_blank" rel="noopener noreferrer">`, plus "Sao chép link".
   - Input "Mã xác nhận" + button "Xác nhận" + "Huỷ".
   - Success → toast "✓ Đã đăng nhập Claude Code CLI" and re-test the provider (green dot). Failure → show the message and
     output tail; allow retry.
   - Also poll `claude auth status` every 5 s while the panel is open (some flows complete without pasting a code).
5. Remove the old "open a console window" login path (keep nothing that calls `cmd /C start ... auth login`).
6. Unit tests: URL extraction from the sample output above (incl. the "Opening browser…" line and prompt), host allowlist,
   code validation.
Update README (Đăng nhập Claude Code CLI) and docs/SPEC.md. Reply per CODEX_RULES.md.
