# Tái cấu trúc AI Hub R1–R4

Trạng thái: hoàn tất · Ngày: 2026-11-10

## Kết quả

- R1: chuẩn hóa tài liệu và scripts kiểm tra.
- R2: tách router/API, nghiệp vụ, nền tảng dùng chung và provider; xóa `src/server.rs` sau khi chuyển toàn bộ tuyến.
- R3: tách giao diện thành ES modules và CSS riêng; `build.rs` nhúng tài nguyên lúc build.
- R4: xóa alias API `/api/orchestration*`, đổi tên tham chiếu giao diện sang MutiS, dịch lỗi API sang tiếng Việt, cập nhật smoke routes, SPEC và README. Giữ bí danh cấu hình Serde `orchestration` để đọc tệp cũ và chuyển nhật ký cũ nếu có.

## Tóm tắt kích thước

| Khu vực | Trước | Sau |
| --- | ---: | ---: |
| Backend | `src/server.rs`: 3.385 dòng | Đã xóa sau khi chia vào `src/api/`, `src/features/`, `src/core/` và `src/providers/` |
| Giao diện | `ui/app.js`: 1.778 dòng | 11 module trong `ui/js/`, tổng 2.323 dòng; CSS nằm riêng trong `ui/styles/` |
| Dòng dài hơn 200 ký tự | Server: 19; giao diện: 64 JS, 24 HTML, 11 CSS | 0 trong các tệp đã rà soát ở các bước R2/R3 |

## Xác minh

R4: `CARGO_TARGET_DIR=target/codex cargo build --release --offline`, `scripts/verify.ps1` và `scripts/smoke.ps1`.
