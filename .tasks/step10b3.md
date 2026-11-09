Step 10b3 — fix live-test findings of 10b2. FIRST read .tasks/RULES.md. Design: plans/mutis.md.

Reviewer tested in the browser with a copy of the real user config (Codex CLI connected; Claude CLI found but NOT logged in;
Anthropic/OpenAI/Deploy keys empty). Observed:
1. Popover "Đội MutiS" showed "Làm việc lớn: Codex CLI ⚠ chưa kết nối — sẽ dùng Claude Code CLI" although Codex IS connected.
   Cause: the provider connection cache is only filled when Settings opens; unknown status is treated as "not connected".
   Fix: three states — connected / not connected / unknown. Unknown shows a grey dot + "đang kiểm tra…" and triggers the
   provider tests (reuse testAllProviders / the 60s server cache) when the popover opens and on page load when MutiS is the
   selected source. Only show the ⚠ fallback line for a confirmed not-connected / not-configured provider.
2. Role "Làm việc nhỏ" was EMPTY after migration (chip showed "—"). Migration/defaults must set small = the deepseek_agent
   provider (id `deepseek-agent-deploy`, its first model) when it exists; never leave a role with an empty provider.
3. Fallback chose unusable providers ("sẽ dùng Anthropic" with no key; "sẽ dùng Claude Code CLI" which is not logged in).
   Fallback must only pick providers whose last test is connected; order per role: lead → claude_cli, anthropic, codex_cli
   (read-only); big → codex_cli, claude_cli; small → deepseek_agent, codex_cli. If nothing connected: "⚠ Không có nguồn
   nào đang kết nối cho vai trò này" and planning/running is blocked with a clear message.
4. Lead default after migration was Anthropic (no key). Default/migrated lead = first CONNECTED in the lead order; if the
   statuses are unknown at load time, keep the configured value but resolve the fallback at run time with live tests.
5. Selecting "✦ MutiS (3 AI)" closes the popover; keep it open and scroll to "Đội MutiS" so the user sees the team.
Unit tests for: role fallback with connected/not/unknown statuses, migration never leaving empty roles.
Update plans/mutis.md progress log. Reply per RULES.md.
