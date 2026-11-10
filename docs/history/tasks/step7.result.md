Đã triển khai mục **Tiện ích** với CodeGraph, xử lý các lỗi review của bước 6, và cập nhật README cùng SPEC. Tôi không cài CodeGraph hay chạy bất kỳ lệnh tiện ích nào.

- Thêm catalog extension ở [extensions.rs](/C:/TOOL/ai-hub/src/extensions.rs), endpoint trạng thái, hành động SSE và dừng tiến trình ở [server.rs](/C:/TOOL/ai-hub/src/server.rs). Hành động chỉ dùng lệnh cố định trong catalog; có xác nhận trước khi chạy, log output, sao lưu cấu hình, giới hạn 10 phút và kết thúc cây tiến trình khi dừng hoặc hết giờ.
- Thêm trang Tiện ích toàn chiều rộng, thẻ trạng thái, liên kết homepage mở tab mới và Nhật ký trong [index.html](/C:/TOOL/ai-hub/ui/index.html), [app.js](/C:/TOOL/ai-hub/ui/app.js) và [style.css](/C:/TOOL/ai-hub/ui/style.css).
- Chuẩn hóa path discovery trên Windows, bỏ ứng viên CodeGraph npm-bundled trùng phiên bản với `codex.cmd`, và sửa thông báo trạng thái provider trong [server.rs](/C:/TOOL/ai-hub/src/server.rs).
- Cho phép dùng lại hàm kết thúc cây tiến trình ở [cli_support.rs](/C:/TOOL/ai-hub/src/providers/cli_support.rs).
- Cập nhật [README.md](/C:/TOOL/ai-hub/README.md) và [SPEC.md](/C:/TOOL/ai-hub/SPEC.md); thêm unit test cho catalog, lọc ANSI và định dạng timestamp.

**Verification:** `cargo test --offline` đạt 3/3; `CARGO_TARGET_DIR=target-codex cargo build --release --offline` thành công; `CARGO_TARGET_DIR=target-codex cargo clippy --offline` thành công, không cảnh báo.