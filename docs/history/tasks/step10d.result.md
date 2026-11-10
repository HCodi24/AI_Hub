**Bước 10d chưa hoàn tất.** Tôi đã làm một phần rồi ghi rõ phần còn thiếu trong [plans/mutis.md](C:/TOOL/ai-hub/plans/mutis.md).

Đã thay đổi:

- Thêm `worker_rules` với nội dung mặc định và ô chỉnh sửa trong Cài đặt MutiS. Quy tắc được thêm vào prompt của task kế hoạch khi MutiS bật.
- Khi verify FAIL, luồng có thể tạo task `Sửa lỗi kiểm tra: <title>` cho cùng assignee nếu auto-fix còn vòng, rồi chạy task đó qua verify và review.
- Thêm `GET /api/mutis/stats?days=30` để tổng hợp một số số liệu từ nhật ký còn lưu.
- Thêm unit test cho helper che API key và mở rộng kiểm tra chuyển trạng thái verify FAIL.
- Cập nhật README, SPEC và nhật ký tiến độ.

**Chưa làm:** trang MutiS và bộ lọc sự kiện; chẩn đoán tự động qua SSE, lưu/lịch sử diagnosis và nút áp dụng thay đổi có whitelist; chuẩn hóa top 5 lỗi; đầy đủ nhóm thống kê theo loại task; unit test cho thống kê, phân tích diagnosis và whitelist. Vì vậy tài liệu SPEC hiện mô tả cả hành vi mục tiêu chưa được triển khai.

**Kiểm tra đã chạy:** `cargo build --release --offline`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline` (34 tests passed), `node --check ui/app.js`, `git diff --check` và quét encoding — đều qua. Không gọi AI hoặc API thật.

`git status` cũng cho thấy `.tasks/step10d.md` đã bị sửa; tôi không chỉnh tệp đó.