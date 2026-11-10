# Tự phát triển (Self-dev) — dùng AI Hub để sửa chính AI Hub

Trạng thái: đang làm · Bắt đầu: 2026-11-10 · Giám sát: Claude · Code: Codex (gpt-6-luna)

## Mục tiêu
AI Hub chỉ có một người dùng (chủ repo). Thêm chế độ để dùng chính app đang chạy sửa mã nguồn của nó:
sửa (chat/Chia việc/MutiS) → build bản thử → chạy thử song song → ổn thì **áp dụng** thành exe chính; hỏng thì **khôi phục**.

## Vấn đề kỹ thuật cần giải
- Windows khoá file exe đang chạy → không build đè `target/release/ai-hub.exe` được.
  → Build bản thử vào `target-dev/` (CARGO_TARGET_DIR riêng).
- Bản thử không được đụng dữ liệu thật → chạy với `AI_HUB_CONFIG_PATH` trỏ tới **bản sao** cấu hình trong
  `%APPDATA%\ai-hub\dev-sandbox\config.json` (chats/plans/mutis của bản thử nằm cạnh đó), cổng riêng (mặc định 7879).
- Thay exe đang chạy: Windows cho phép **đổi tên** exe đang chạy → đổi `ai-hub.exe` → `ai-hub.prev.exe`, chép bản mới
  vào `ai-hub.exe`, khởi động bản mới (cùng cổng) rồi thoát bản cũ; lỗi → đổi tên ngược lại.
- An toàn: checkpoint git trước khi sửa; không cho áp dụng khi build/test/verify chưa đạt.

## Trang "Tự phát triển" (thanh bên, chỉ hiện khi bật trong Cài đặt)
1. Dự án: đường dẫn repo của app (tự nhận từ vị trí exe: `<repo>\target\release\ai-hub.exe` → `<repo>`; sửa được).
   Hiện nhánh git, số file thay đổi, commit cuối.
2. "Sửa app bằng AI": nút mở Chat mới / Chia việc với Thư mục = repo (MutiS nếu bật).
3. "Build bản thử": `cargo build --release` vào `target-dev`, log trực tiếp, rồi `cargo test` + cổng kiểm tra.
4. "Chạy bản thử": chạy `target-dev\release\ai-hub.exe --port 7879 --no-open` với cấu hình sandbox, nút mở tab; dừng.
5. "Áp dụng bản mới" (chỉ bật khi build+test+verify đạt và bản thử đã chạy được): xác nhận → thay exe chính → app tự
   khởi động lại trên cổng hiện tại; trang tự tải lại.
6. "Khôi phục bản trước": dùng `ai-hub.prev.exe`.
7. Git: "Tạo checkpoint" (commit WIP), "Xem thay đổi" (diff), "Bỏ thay đổi về checkpoint" (xác nhận), "Commit" (tác giả theo
   git config của repo — HCodi24).

## Các bước
- [ ] 11a — cấu hình `self_dev` + trang Tự phát triển: dự án/git status, build bản thử (target-dev) + test + verify,
  chạy/dừng bản thử trên cổng 7879 với cấu hình sandbox.
- [ ] 11b — áp dụng bản mới (đổi tên exe + khởi động lại), khôi phục, checkpoint/diff/bỏ thay đổi/commit; khoá an toàn.

## Quyết định
- Chỉ chạy local, chỉ người dùng hiện tại; không thêm đăng nhập.
- Không tự động áp dụng; luôn cần người dùng bấm + xác nhận.
- Bản thử luôn dùng dữ liệu sandbox (bản sao), không đụng dữ liệu thật.

## Nhật ký tiến độ
