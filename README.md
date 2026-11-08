# AI Hub

AI Hub là ứng dụng Rust chạy cục bộ, cung cấp giao diện tiếng Việt để trò chuyện với các API tương thích OpenAI và Anthropic.

## Biên dịch và chạy

Cài Rust, mở PowerShell tại thư mục dự án rồi chạy:

```powershell
cargo build --release
cargo run --release
```

Ứng dụng lắng nghe tại `127.0.0.1:7878` và tự mở trình duyệt. Dùng `--no-open` để bỏ qua việc mở trình duyệt, hoặc `--port 9000` để đổi cổng. Có thể đặt cổng mặc định bằng `$env:AI_HUB_PORT=9000` trong PowerShell.

## Cấu hình và lịch sử chat

Cấu hình nằm tại `<config_dir>/ai-hub/config.json` (trên Windows là `%APPDATA%\ai-hub\config.json`). Các cuộc trò chuyện được lưu thành tệp JSON trong `<config_dir>/ai-hub/chats/`.

API key được che trong phản hồi cài đặt và không được ghi vào log. Ở bước 1, Codex CLI và Claude Code CLI chỉ được phát hiện phiên bản; gửi chat qua CLI sẽ báo chưa hỗ trợ.
