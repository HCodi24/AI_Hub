## Hoàn tất R4

Đã xóa hai route alias `/api/orchestration*` và đổi các tham chiếu orchestration trong giao diện sang MutiS. Các lỗi cố định trong API và feature đã được dịch sang tiếng Việt; smoke test cũng không còn gọi route cũ. Bí danh Serde `#[serde(alias = "orchestration")]` vẫn được giữ để đọc cấu hình cũ. Migration nhật ký vẫn dùng `old.exists()`, nên thư mục `orchestration/` không có mặt thì không gây lỗi.

Đã viết lại [docs/SPEC.md](/C:/TOOL/ai-hub/docs/SPEC.md) theo kiến trúc hiện tại, cập nhật phần cấu trúc mã nguồn và chế độ dev trong [README.md](/C:/TOOL/ai-hub/README.md), đồng thời đánh dấu R1–R4 hoàn tất trong [docs/design/refactor.md](/C:/TOOL/ai-hub/docs/design/refactor.md). Bản tóm tắt ghi nhận backend `src/server.rs` từ 3.385 dòng đã được chia sang các module mới và xóa; giao diện `ui/app.js` từ 1.778 dòng được tách thành 11 module trong `ui/js/` với tổng 2.323 dòng.

Các tệp mã đã cập nhật: [src/api/mod.rs](/C:/TOOL/ai-hub/src/api/mod.rs), [src/api/dev.rs](/C:/TOOL/ai-hub/src/api/dev.rs), [src/api/extensions.rs](/C:/TOOL/ai-hub/src/api/extensions.rs), [src/api/mutis.rs](/C:/TOOL/ai-hub/src/api/mutis.rs), [src/api/plans.rs](/C:/TOOL/ai-hub/src/api/plans.rs), [src/api/providers.rs](/C:/TOOL/ai-hub/src/api/providers.rs), [src/api/chat.rs](/C:/TOOL/ai-hub/src/api/chat.rs), [src/features/mutis.rs](/C:/TOOL/ai-hub/src/features/mutis.rs), cùng giao diện tại `ui/index.html`, `ui/js/` và `ui/styles/settings.css`. Danh sách route trong [scripts/smoke.ps1](/C:/TOOL/ai-hub/scripts/smoke.ps1) đã được cập nhật.

## Kết quả kiểm tra

- `CARGO_TARGET_DIR=target/codex cargo build --release --offline` — đạt.
- `powershell -ExecutionPolicy Bypass -File scripts/verify.ps1` — đạt: fmt, clippy, test, node, encoding và line-length.
- `powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1` — đạt, mã thoát 0.

**Chưa hoàn tất:** Không có.