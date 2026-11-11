Step U1 of AI Hub — visual polish of the React UI (web/) using the design system. FIRST read docs/CODEX_RULES.md,
docs/FE_STRUCTURE.md, design-system/ai-hub/MASTER.md, and these UI/UX rule files (read on demand, apply them):
C:/Users/nts68/.claude/plugins/cache/ui-ux-pro-max-skill/ui-ux-pro-max/2.15.0/.claude/skills/ui-ux-pro-max/references/quick-reference.md
C:/Users/nts68/.claude/plugins/cache/ui-ux-pro-max-skill/ui-ux-pro-max/2.15.0/.claude/skills/ui-ux-pro-max/references/pro-rules.md
Repo C:\GitHub\AI_Hub. No network; packages already installed (incl. @fontsource-variable/inter and
@fontsource-variable/jetbrains-mono). Rust untouched unless needed. No real AI calls. Stop any server you start. UTF-8 + LF only;
Vietnamese intact. Do not touch _restructure_ui.py or ui/ (legacy).

Product: a desktop AI orchestration chat app (Vietnamese), soon packaged with Tauri. Users: developers. Feel: precise, calm,
premium developer tool (think Linear / Raycast / Claude desktop), dark-first with a good light theme. NOT a marketing page —
ignore the "FAQ/Documentation Landing" pattern in MASTER.md; use its style, color, typography, effects and checklist.

1. Tokens (web/src/styles/tokens.css): adopt MASTER.md as semantic tokens (background, foreground, card, card-foreground,
   muted, muted-foreground, border, primary/on-primary, accent (green, for run/success)/on-accent, destructive, warning,
   info, ring, plus surface-raised/overlay, selection) for dark (default) and light (`[data-theme="light"]` +
   prefers-color-scheme). Verify text contrast ≥ 4.5:1 for body/muted text in BOTH themes (compute it; adjust values; write
   the ratios as a comment). Spacing 4/8/12/16/20/24/32/48, radius 6/8/12, elevation 1–3 (subtle, sharp), durations 150/200/
   250ms + easing tokens, z-index scale. No raw hex in component CSS — tokens only (grep and fix).
2. Typography: Inter Variable for UI, JetBrains Mono Variable for code/logs (import the @fontsource-variable CSS in _app);
   base 15–16px, line-height 1.5 (1.6 for markdown), clear scale (12/13/14/15/16/18/20/24), tabular numbers for timers/tokens.
3. Layout polish: sidebar (240–280px, collapsible, ☰ drawer < 768px with overlay + Esc + focus trap), top bar compact (source,
   model, permission, effort as one "mode" control group; folder as a path chip with full path tooltip), message column max
   width ~760–820px centered, composer floating at the bottom with clear send/stop states, empty state with 3–4 suggestion
   chips. Assistant messages without bubble (full-width prose), user messages as subtle bubbles right-aligned.
4. Components: one consistent button system (primary/secondary/ghost/danger, sizes sm/md, icon-only with aria-label and ≥
   36×36 hit area desktop / 44×44 under 768px), select, chip/badge, card, tabs, collapsible, tooltip-like title, toast,
   dialog. Lucide icons only (no emoji in UI chrome: replace ⚖ ✦ 📝 ▶ ✓ ✗ ○ ◐ with Lucide icons: Scale, Sparkles, NotebookPen,
   Play, CheckCircle2, XCircle, Circle, Loader2 spin…); decorative icons aria-hidden.
5. Cards (debate, plan, memory): unify into the card language — header row (icon + title + status pill + timer right), body,
   footer actions right-aligned; status colors from tokens (running = info + spinner, done = accent, failed = destructive,
   draft = muted). Plan task rows: compact, hover highlight, expand chevron, progress bar for N/M done.
6. Markdown/code: code blocks with header (language + Copy with "Đã chép" feedback), JetBrains Mono, horizontal scroll inside
   the block only, highlight.js theme matching each app theme (github-dark / github styles from highlight.js package via
   CSS import or token-mapped), tables with zebra + borders, blockquote, inline code chip, links accent.
7. Motion: 150–250ms transitions on hover/focus/expand (opacity/transform only), streaming caret, skeletons for loading
   history/cards, respect prefers-reduced-motion. Focus-visible rings everywhere (2px ring token).
8. Performance: memo message rows, keep virtualization, avoid re-rendering the whole list on each delta (store selectors),
   no layout shift when cards load (reserve min-height).
9. Theme toggle in the sidebar footer (Sáng / Tối / Theo hệ thống), persisted.
10. Run the pre-delivery checklist from pro-rules.md + MASTER.md and report each item (pass / fixed / not applicable).
Verify: web typecheck/lint/build + node scripts/test-web.cjs + scripts/test-web-r2.cjs + scripts/verify.ps1 + smoke.ps1.
Reply per docs/CODEX_RULES.md (files, before/after summary, checklist results, verify output, anything not done).
