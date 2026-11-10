Đã hoàn tất Step 11c — **Chat phát triển**.

Nút **⚒ Chat phát triển** xuất hiện trong dev mode và dùng chung hàm mở chat với nút trên trang **Tự phát triển**. Chat mới ghim folder vào repo, đặt quyền sửa file, chọn MutiS khi bật hoặc Codex CLI với effort `high` khi tắt; folder hiện chỉ đọc. Cờ `dev_chat` được lưu trong chat và plan, biểu tượng ⚒ xuất hiện trong danh sách chat, và preamble được thêm vào tin nhắn đầu tiên cùng các plan-task prompt. Khi dev mode tắt, chat cũ vẫn dùng folder đã lưu và hiển thị thông báo tương ứng. Server có unit test cho quy tắc chọn folder.

**Đã xác minh:** `cargo test --offline` với `CARGO_TARGET_DIR=target-codex` — 42 test đạt; `cargo clippy --offline --all-targets -- -D warnings` đạt; `node --check ui/app.js` đạt; `git diff --check` và quét chuỗi mojibake đạt.

**Chưa làm:** Không chạy `cargo build --release` theo chỉ dẫn phiên này. Không gọi AI thật; chưa kiểm tra luồng tương tác trên trình duyệt.

Đã cập nhật [README.md](/C:/TOOL/ai-hub/README.md), [plans/self-dev.md](/C:/TOOL/ai-hub/plans/self-dev.md), [src/chat.rs](/C:/TOOL/ai-hub/src/chat.rs), [src/plans.rs](/C:/TOOL/ai-hub/src/plans.rs), [src/server.rs](/C:/TOOL/ai-hub/src/server.rs), [ui/app.js](/C:/TOOL/ai-hub/ui/app.js), [ui/index.html](/C:/TOOL/ai-hub/ui/index.html) và [ui/style.css](/C:/TOOL/ai-hub/ui/style.css).