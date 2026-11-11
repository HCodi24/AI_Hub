Step R1 of AI Hub — new React frontend (foundation + first chat screen). FIRST read docs/CODEX_RULES.md and docs/SPEC.md.
Repo: C:\GitHub\AI_Hub. Rust: CARGO_TARGET_DIR=target/codex, `--offline`. No network: dependencies are ALREADY installed in
web/node_modules (do not add packages; if something is truly missing, stop and list it). No real AI calls. Never stop ai-hub
processes. UTF-8 + LF only (never PowerShell Set-Content/Out-File); keep Vietnamese intact. Do not touch _restructure_ui.py.

DECISION (user): the new UI must follow EXACTLY the folder structure/conventions of the user's project
C:\GitHub\BEANS\SCM-BEANS_Dashboard_FE (read it — src/ layout, naming, http/api/schema pattern, CSS modules — but do NOT read
.env*, .npmrc or any secrets, and do not copy business code). Summary of that structure:
  web/ (Next.js 16 Pages Router, React 19, TypeScript strict, pageExtensions ["page.tsx","page.ts"], path alias @/* → src/*)
  src/apis/<name>.api.ts        — one object per backend area (fetch/axios-like client from src/lib/http.ts), parse with zod
  src/schema/<name>.schema.ts   — zod schemas + inferred types
  src/lib/                      — http.ts (base client: same-origin "/api", JSON, error → ApiError with Vietnamese message),
                                  sse.ts (POST + text/event-stream reader, AbortSignal), markdown.ts, utils (cn = clsx+twMerge)
  src/routes/path.ts, routes.ts — route constants
  src/constants/, src/text/     — Vietnamese UI strings live in src/text/<area>.ts (no hard-coded strings in components)
  src/components/<kebab-name>/<kebab-name>.tsx + <kebab-name>.module.css   (shared UI: button, icon-button, select, badge,
                                  sidebar, mobile-navigation, markdown-view, code-block, spinner, empty-state, dialog…)
  src/components/features/<feature>/   (feature UI: chat, composer, debate, plan, memory, settings, mutis, dev…)
  src/pages/_app.page.tsx (QueryClientProvider like the reference), src/pages/index.page.tsx, src/pages/chat/… screens
  src/styles/ globals.css + tokens.css (design tokens: spacing 4/8/12/16/24/32, radius, shadow, font sizes, colors for dark AND
  light via [data-theme] / prefers-color-scheme; Inter-like system font stack + mono stack; no external font fetch).
  State: @tanstack/react-query for server data, zustand for UI/chat streaming state. Icons: lucide-react (no emoji in UI chrome).
  Tailwind is installed but use CSS Modules + tokens like the reference (tailwind only if the reference uses it for utilities).

BUILD/SERVE
1. web/next.config.js: output "export", basePath "/app", trailingSlash true, images unoptimized, reactStrictMode, distDir
   default; export goes to web/out. tsconfig like the reference (strict, bundler resolution, @/*). .eslintrc.json like the
   reference (next/core-web-vitals). web/.gitignore: node_modules, .next, out. Add web/README.md (how to dev/build).
   Root .gitignore: web/node_modules, web/.next, web/out.
2. Rust (src/api/static_ui.rs + router in src/api/mod.rs): serve the export under /app/* from `<repo or exe dir>/web/out`
   (resolve the same way the current ui/ dir is resolved for dev and release; if the release currently embeds ui/, embed
   web/out the same way ONLY when it exists at build time, else serve from disk; never fail to start when web/out is missing —
   then /app shows a small Vietnamese page "Giao diện mới chưa được build: chạy npm run build trong web/"). Correct MIME types
   for .js/.css/.woff2/.svg/.json/.txt, `/app/` → index.html, unknown /app/* paths → /app/404.html or index.html.
   The legacy UI stays at "/" unchanged in this step. Add a small link in the legacy UI sidebar bottom: "Giao diện mới (beta)"
   → /app/.
3. scripts/verify.ps1: new checks `web-typecheck` (npx tsc --noEmit in web/), `web-lint` (npm run lint), `web-build`
   (npm run build) — skipped with a warning line when web/node_modules is missing. smoke.ps1: GET /app/ returns 200 html when
   web/out exists.

FIRST SCREEN (prove the stack end-to-end; match current behaviour, nicer look)
4. Layout: left sidebar (chat history from GET /api/chats, "Chat mới", collapses to a ☰ drawer under 768px), main column with
   a top bar (source/model select summary + folder input), message list (react-virtuoso), composer at the bottom (auto-growing
   textarea, Enter send / Shift+Enter newline, Stop button while streaming).
5. Chat: load a chat (GET /api/chats/{id}), send via POST /api/chat (same request body as legacy ui/js/chat.js — read it and
   the Rust ChatRequest; zod schemas for ChatRecord/ChatMessage/ProviderSummary/config), stream SSE events delta/reasoning/
   log/diff/error/status/heartbeat/memory/done into a zustand store; render assistant markdown with marked + DOMPurify +
   highlight.js (code blocks with language label + Copy button; tables, lists, headings, links open in new tab), hide
   <memory>…</memory> blocks, collapsible "Hoạt động" log + token line, diff block (green/red lines), error box.
   Messages with debate_id / plan_id: show a simple placeholder card "Tranh luận / Kế hoạch — mở trong giao diện cũ" with a
   link to "/" (full cards come in later steps).
6. Sources/models: GET /api/config (providers), provider + model + permission (read/write/full) + effort selects in the top bar
   (behaviour like legacy mode chip, simplified); folder input saved per chat like legacy.
7. Quality: no `any`; components small (< ~250 lines); every user-visible string from src/text; accessible (labels,
   focus-visible, aria-live for streaming); dark + light; 375px works (no horizontal scroll).
8. Docs: README section "Giao diện mới (web/)" + SPEC (serving /app, build commands). Add docs/FE_STRUCTURE.md describing the
   folders/conventions above so later steps follow them.
Verify: `npm run typecheck`, `npm run lint`, `npm run build` in web/, then scripts/verify.ps1 + scripts/smoke.ps1.
Reply per docs/CODEX_RULES.md (files, what works, verify output, anything not done).
