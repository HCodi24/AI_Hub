# MutiS-owned Codex environment

Status: discovery complete; implementation added. Live account sign-in and
inference verification remain pending.

## User and outcome

MutiS users connect Codex without operating a terminal. Codex uses app-owned
configuration and context, while preserving existing MutiS chat history and
project memory. The user delegates the integration technology and code
organization to the agent, subject to the confirmed behavior below.

## Confirmed product decisions

- Settings > AI Sources has two distinct actions: Find Codex CLI and Browser
  sign-in. CLI discovery attempts to reuse an existing signed-in account where
  supported. Browser sign-in opens the official sign-in flow.
- Both connection methods use the MutiS-owned Codex environment. Reusing a login
  does not authorize importing personal rules, plugins, skills or MCP settings.
- Project skills and project instructions (AGENTS.md) have separate settings,
  both off by default. Enabling either must not implicitly enable the other or
  project MCP servers and hooks.
- MutiS chat history and project memory remain owned by MutiS, outside Codex home.
- English-only rule authoring applies to development of MutiS itself.

## Starting state and evidence

- Runtime: Rust backend, React/Next frontend, Windows desktop environment.
- Current chat adapter: src/providers/codex_cli.rs launches codex exec through
  src/providers/cli_support.rs; no dedicated Codex home is set there.
- src/api/providers/login.rs currently implements Claude login only.
- Settings provider UI lives in
  web/src/components/features/settings/provider-form.tsx.
- Existing CLI discovery scans configured paths, PATH, registry PATH and known
  installation directories. A missing shell PATH command is not evidence that
  Codex is not installed.
- Found executable:
  C:/Users/nts68/AppData/Local/OpenAI/Codex/bin/5449c1f0f26f1f98/codex.exe
- Executable --version returned codex-cli 0.167.0-alpha.3.2.
- Executable app-server --help confirmed stdio transport, configuration overrides
  and JSON-schema generation. This is a capability check, not a successful login
  or chat verification. Both commands warned that the current tool environment
  could not find a home directory.
- Saved app memory settings inspected: enabled=true, max_scopes_in_prompt=3,
  max_chars_per_scope=6000. No credentials were printed.
- src/features/memory/inject.rs injects project core context into new CLI sessions.
  src/providers/cli_support.rs has a limited recent-conversation fallback when no
  session exists. Memory is a summary, not a complete Codex session snapshot.

## Integration direction after delegation

Use a dedicated Codex adapter backed by app-server over stdio for both connection
methods. Keep protocol and process types inside the adapter; API handlers and UI
consume app-owned connection and chat contracts. Keep configuration/home handling,
authentication and runtime session handling separate within that boundary.

Compared alternatives:

| Direction | Benefit | Cost / reason |
| --- | --- | --- |
| app-server for connection and chat | One session lifecycle and documented browser-login events | Selected; requires adapting existing exec event handling and testing the installed alpha protocol |
| exec chat plus separate auth transport | Smaller initial chat change | Not selected; adds two transports to maintain and coordinate |
| personal CLI environment unchanged | Minimal integration change | Rejected; conflicts with isolated rules, skills and MCP |

Official references:

- https://learn.chatgpt.com/docs/app-server
- https://learn.chatgpt.com/docs/auth
- https://learn.chatgpt.com/docs/config-file/config-advanced
- https://learn.chatgpt.com/docs/build-skills

## Boundaries and constraints for specification

- Use an app-data Codex home; never redirect the user's global environment or
  modify personal Codex configuration as part of app startup.
- CODEX_HOME alone is not full source isolation. Account for project config,
  AGENTS.md, user/repository/admin skill discovery, plugins, hooks and inherited
  environment. Respect mandatory platform/organization policy; do not claim an
  entirely empty model context or removal of Codex's built-in instructions.
- Only app-authorized optional sources should appear in the model context.
  Setting changes must not leave a resumed session carrying previously enabled
  sources. Define an environment revision/session invalidation mechanism.
- Authentication reuse must be supported and verifiable. Do not silently fall
  back to running with the personal Codex home. If reuse is unavailable, return
  an actionable status pointing to browser sign-in.
- Store credentials only through the selected supported credential mechanism.
  Never send access/refresh tokens to the frontend, logs or memory.
- Distinguish MutiS sign-out from any effect on the user's standalone CLI login.
  Establish credential ownership and refresh behavior before implementing reuse.
- Preserve existing chat records and memory. When an old native session is not
  valid for the new environment, create a new native session and seed app-owned
  context instead of blindly resuming the old session ID. Do not promise exact
  recovery of internal reasoning or unsaved CLI state.
- Plain chat currently disables project memory; preserve that behavior unless
  separately requested. Unattended plan workers receive planner-owned context.
- Keep connection status shared with the existing Settings/source-menu cache.
- No unrelated provider, MutiS role or attachment redesign is in scope.

## Observable acceptance criteria

1. Find Codex CLI discovers the installed executable, reports its version and
   produces a clear connected or authentication-unavailable result.
2. Browser sign-in opens the official browser flow, reports success/cancel/error,
   and persists the app's connection across a backend restart where supported.
3. A fresh app session does not expose seeded personal AGENTS, skills, plugins or
   MCP markers. Verify actual context/tool exposure, not only environment variables.
4. Each project-source switch operates independently; both default to off, persist
   across reloads, and apply to newly initialized runtime context.
5. Existing MutiS chat history and memory survive the transition. A continuing
   project chat receives memory and relevant app history in its new runtime session.
6. Chat streaming, cancellation, error reporting, usage and file-change reporting
   remain compatible with the current UI. Test both connection methods.
7. Failed auth, unsupported protocol and process exit do not leave a false
   connected status or orphaned processes. Tokens never appear in returned errors.
8. Settings and repeated source-menu opening share the same connection result.

## Assumptions and remaining technical validation

- No unresolved user product selection remains. The agent may choose implementation
  details within the delegated boundaries without repeating product questions.
- Credential reuse compatibility, exact installed app-server schema, external
  skill-source controls and live OAuth/chat remain unverified.
- Old-session recovery behavior is the technical recommendation derived from the
  existing memory implementation, not a claim that memory captures every old turn.
- Recommend tools/tool-integration-design with backend and frontend support.
- Initial implementation now lives in src/providers/codex/ and
  src/api/providers/codex.rs, with UI in codex-connection.tsx.
- Both source switches are global to the app's Codex providers in this initial UI.
  They are independent and default to off. Their contents are explicitly injected
  under a bounded size budget rather than granting access to project MCP/hooks.
- CLI authentication reuse currently supports file-based ChatGPT auth caches.
  Keyring-only or incompatible caches return an actionable browser-sign-in message.
- App-server handshake, isolated signed-out state, read-only thread creation,
  browser auth URL generation and cancellation passed against the installed binary
  using a disposable home, without inference or personal credentials.
- Actual browser sign-in, token refresh and streamed model replies still require
  live verification after loading the updated backend.
