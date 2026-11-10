Step I1 of AI Hub — image attachments in the chat composer. FIRST read docs/CODEX_RULES.md and docs/SPEC.md.
Build with CARGO_TARGET_DIR=target/codex. Gate: scripts/verify.ps1 + scripts/smoke.ps1. No real AI calls.

UI (ui/js/chat.js + composer markup/styles):
1. Add images by: paste (Ctrl+V), drag & drop onto the composer, and a "📎" button (file picker, accept image/*).
   Max 5 images per message, max 10 MB each, types png/jpeg/webp/gif. Show thumbnails above the textarea with an ×
   to remove. Send is allowed with images and no text.
2. Sent user messages show the thumbnails in the message list (click → open full size in a new tab).

Backend:
3. `POST /api/attachments` (multipart or JSON base64 — your choice; validate type by magic bytes, size limits) saves to
   `<data_dir>/attachments/<chat_id>/<uuid>.<ext>` and returns {id, url, mime, bytes}. `GET /api/attachments/{chat}/{file}`
   serves it (path-safe). Chat messages get optional `attachments: [{id, mime, path}]` persisted in the chat record.
4. Providers:
   - claude_cli: pass `--add-dir <attachments dir of this chat>` and append to the prompt (stdin) lines
     "Ảnh đính kèm: <absolute path>" (one per image) + "Hãy dùng công cụ đọc tệp để xem ảnh trên." .
   - codex_cli: add `-i <path>` per image (it is supported by `codex exec` and `exec resume`; validate the path is inside
     the attachments dir; pass via the safe argv builder).
   - anthropic: content blocks `{type:"image", source:{type:"base64", media_type, data}}` before the text block.
   - openai_compat: content array `[{type:"text",text}, {type:"image_url", image_url:{url:"data:<mime>;base64,..."}}]`;
     if the API answers 400 mentioning image/vision/content, emit error "Model này không hỗ trợ ảnh".
   - deepseek_agent: reject with error "DeepSeek làm việc chưa hỗ trợ ảnh — hãy dùng nguồn khác".
   - MutiS normal chat uses the lead provider rules above.
5. Older messages: only the latest user message's images are sent to CLIs (they keep history themselves); API
   providers resend images of earlier user messages only if total payload < 15 MB (else drop oldest with a log line).
6. Unit tests: magic-byte validation, path safety for serving and for codex -i, openai/anthropic message builders.
Update README (Chat) + docs/SPEC.md. Reply per CODEX_RULES.md.
