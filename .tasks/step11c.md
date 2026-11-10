Step 11c of AI Hub — "Chat phát triển" button. FIRST read .tasks/RULES.md and plans/self-dev.md.
NOTE: the user is running the app in dev mode right now from this repo; your UI edits will live-reload in their
browser. Never stop/kill ai-hub processes and never run `cargo build --release` (use CARGO_TARGET_DIR=target-codex as usual).

Goal (user request): a dedicated button that opens a normal chat which is a "self-development" chat — it always works on
the AI Hub repo folder.

1. Chat record + request: optional `dev_chat: bool` (serde default false).
2. Sidebar: button "⚒ Chat phát triển" right under "Chat mới", visible only in dev mode. Click → create a new chat with
   dev_chat = true, folder = repo path from /api/dev/repo, permission "write", source = MutiS when enabled, otherwise
   Codex CLI (its configured model; effort "high"), composer placeholder "Mô tả thay đổi cho AI Hub…".
   The existing "Mở chat sửa app" button on the Tự phát triển page must call the same function.
3. In a dev chat:
   - a header strip "⚒ Chat tự phát triển · thư mục: <repo path>" (and the folder input is read-only for that chat);
   - chat list items show a ⚒ icon;
   - server-side: when dev mode is on and dev_chat is true, IGNORE the client folder and use the repo path (so the folder
     cannot drift); if dev mode is off, the chat still works with its saved folder but shows "(chế độ dev đang tắt)".
4. Context for the AI: for dev chats, prepend to the user's FIRST message (and to plan-task prompts started from a dev
   chat) a short Vietnamese preamble:
   "Bạn đang sửa chính ứng dụng AI Hub (repo: <path>) đang chạy ở chế độ dev.
    - Giao diện ở ui/ tự tải lại khi lưu; code Rust ở src/ cần người dùng bấm 'Build & khởi động lại'.
    - Không chạy `cargo build --release`, không tắt tiến trình ai-hub, không sửa target/.
    - Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy `cargo test` với CARGO_TARGET_DIR=target\dev-check nếu cần.
    - Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không."
   (MutiS worker_rules still apply on top when MutiS is on.)
5. Unit test: server folder override for dev chats (pure function: (dev_mode, dev_chat, client_folder, repo) -> folder).
Update plans/self-dev.md progress log + README "Chế độ dev". Reply per RULES.md.
