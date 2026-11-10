# AI Hub — đặc tả kiến trúc hiện tại

AI Hub là ứng dụng desktop cục bộ dạng một binary Rust, phục vụ giao diện web trên loopback. Ứng dụng kết nối các nguồn AI HTTP và CLI, lưu cấu hình/lịch sử trên máy người dùng và hỗ trợ chat, lập kế hoạch, MutiS, xác minh, tiện ích và chế độ phát triển.

## Kiến trúc

- `src/main.rs` khởi động ứng dụng, phân tích tùy chọn cổng, tạo trạng thái và mở trình duyệt khi được yêu cầu.
- `src/app.rs` chứa `AppState` và các trạng thái dùng chung. `src/api/mod.rs` lắp router Axum, chia tuyến theo tính năng và kiểm tra Host/Origin.
- `src/api/` chỉ xử lý HTTP: giải mã đầu vào, gọi feature/provider, trả JSON hoặc SSE. `src/features/` chứa nghiệp vụ và lưu trữ, độc lập với Axum. `src/core/` cung cấp cấu hình, đường dẫn, tiến trình và xử lý văn bản dùng chung. `src/providers/` triển khai giao tiếp với từng loại nguồn AI và hỗ trợ dò CLI.
- `ui/index.html`, `ui/styles/` và `ui/js/` tạo giao diện HTML/CSS/ES modules. `build.rs` nhúng các tài nguyên UI vào binary lúc biên dịch; API static phục vụ `/` và đường dẫn tài nguyên đã nhúng.
- `scripts/dev.ps1` chạy chế độ phát triển với UI đọc trực tiếp từ thư mục làm việc. `scripts/verify.ps1` kiểm tra chất lượng/build. `scripts/smoke.ps1` chạy ứng dụng với dữ liệu tạm và gọi các tuyến API chính.

## API theo tính năng

### Giao diện
- `GET /` và `GET /{*path}`: trang và tài nguyên UI đã nhúng.

### Chế độ phát triển (`/api/dev`)
- `GET /api/dev/status`, `/api/dev/repo`, `/api/dev/git/diff`, `/api/dev/events`
- `GET, PUT /api/dev/settings`
- `POST /api/dev/git/checkpoint`, `/api/dev/git/reset`, `/api/dev/git/commit`, `/api/dev/test`, `/api/dev/release`, `/api/dev/rebuild`

### Cấu hình
- `GET, PUT /api/config`

### MutiS
- `GET, PUT /api/mutis`
- `GET /api/mutis/roles/resolve`, `/api/mutis/stats`, `/api/mutis/diagnoses`
- `GET, POST /api/mutis/events`
- `POST /api/mutis/diagnose`, `/api/mutis/apply`, `/api/mutis/setting-change`

### Nhà cung cấp và CLI
- `POST /api/providers`; `PUT, DELETE /api/providers/{id}`
- `GET /api/providers/{id}/models`; `POST /api/providers/{id}/test`, `/api/providers/{id}/use-deploy-key`
- Đăng nhập web Claude Code CLI: `/login/start` chạy `claude auth login --claudeai` ẩn, trả URL HTTPS hợp lệ và session ID; `/login/code` gửi mã web; `/login/cancel` dừng phiên. Mỗi lần chỉ giữ một phiên; phiên hết hạn sau 10 phút không hoạt động và tiến trình bị dừng khi thoát ứng dụng.
- `GET /api/cli-status`, `/api/cli-discover/{name}`

### Chat và lịch sử
- `POST /api/chat`, `/api/chat/{chat_id}/stop`
- `GET /api/chats`, `GET, DELETE /api/chats/{chat_id}`
- `POST /api/attachments` nhận JSON base64; `GET /api/attachments/{chat}/{file}` phục vụ ảnh đã kiểm tra.

Composer hỗ trợ dán, kéo thả và chọn ảnh PNG/JPEG/WebP/GIF, tối đa 5 ảnh mỗi tin nhắn và 10 MB mỗi ảnh. Chat message có trường `attachments: [{id, mime, path}]`; đường dẫn do máy chủ tạo trong `attachments/<chat_id>/`. Claude CLI và Codex CLI chỉ nhận ảnh của tin nhắn người dùng mới nhất; Anthropic và OpenAI-compatible gửi lại ảnh lịch sử nếu tổng dung lượng dưới 15 MB. DeepSeek agent từ chối tin nhắn có ảnh.

### Kế hoạch và xác minh
- `GET, POST /api/plans`; `GET, PUT /api/plans/{id}`
- `POST /api/verify`, `/api/plan-diff`

### Tiện ích
- `GET /api/extensions`
- `POST /api/extensions/{id}/actions/{action}`, `/api/extensions/{id}/stop`

Chat, planner và thao tác tiện ích có thể trả SSE để cập nhật tiến trình. Các tuyến khác trả JSON hoặc mã trạng thái HTTP.

## Dữ liệu

Dữ liệu nằm trong thư mục cấu hình hệ điều hành, dưới `ai-hub/` (Windows thường là `%APPDATA%\ai-hub`):

- `config.json`: cấu hình và khóa API đã lưu cục bộ. Trường cấu hình MutiS là `mutis`; bộ giải mã vẫn chấp nhận bí danh `orchestration` để đọc cấu hình cũ.
- `chats/`, `attachments/`, `plans/`: lịch sử chat, ảnh theo chat và kế hoạch.
- `mutis/events.jsonl`, `mutis/diagnoses/`: sự kiện và kết quả chẩn đoán. Lần khởi động đầu có thể chuyển `orchestration/events.jsonl` cũ sang thư mục MutiS; thiếu thư mục cũ là bình thường.
- `backups/`: bản sao cấu hình được tạo bởi các thao tác tiện ích cần sao lưu.

## Bảo mật

- Máy chủ chỉ bind `127.0.0.1`; middleware chỉ chấp nhận Host loopback cùng cổng và kiểm tra Origin cho yêu cầu không phải GET.
- API key không được đưa vào URL hoặc log. `GET /api/config` trả key đã che; cập nhật với key trống/đã che giữ nguyên key hiện tại.
- CLI nhận prompt qua stdin. MutiS worker bị giới hạn vào các công cụ file được cung cấp; thao tác tiện ích dùng catalog lệnh cố định và yêu cầu xác nhận ở giao diện.
- Đường dẫn làm việc được kiểm tra phía máy chủ. Chức năng thực thi lệnh AI có quyền theo từng task; xác minh/review dùng quyền chỉ đọc theo cấu hình.
