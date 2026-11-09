# AI Hub

AI Hub là ứng dụng Rust chạy cục bộ, cung cấp giao diện tiếng Việt để trò chuyện với API và công cụ lập trình AI.

## Nhà cung cấp

- **OpenAI-compatible:** kết nối các API tương thích với giao thức OpenAI bằng URL, API key và model do bạn cấu hình.
- **Anthropic:** kết nối trực tiếp với Anthropic API bằng API key.
- **Codex CLI:** gửi yêu cầu tới Codex CLI trong thư mục làm việc đã chọn và tiếp tục cuộc trò chuyện qua session.
- **Claude Code CLI:** gửi yêu cầu tới Claude Code CLI trong thư mục làm việc đã chọn và tiếp tục cuộc trò chuyện qua session.

Hai nhà cung cấp HTTP cần API key. Hai nhà cung cấp CLI dùng chương trình CLI đã cài và đăng nhập trên máy này.

### Cài Claude Code CLI

Cài Node.js và npm, sau đó chạy trong PowerShell:

```powershell
npm i -g @anthropic-ai/claude-code
claude
```

Lệnh `claude` mở bước đăng nhập. Đăng nhập một lần trước khi chọn Claude Code CLI trong AI Hub.

## Biên dịch và chạy

Cài Rust, mở PowerShell tại thư mục dự án rồi chạy:

```powershell
cargo build --release
cargo run --release
```

Ứng dụng lắng nghe tại `127.0.0.1:7878` và tự mở trình duyệt. Dùng `--no-open` để bỏ qua việc mở trình duyệt, hoặc `--port 9000` để đổi cổng. Có thể đặt cổng mặc định bằng `$env:AI_HUB_PORT=9000` trong PowerShell.

## Quyền truy cập CLI

Với Codex CLI và Claude Code CLI, chọn **Quyền** cho từng chat:

- **Chỉ đọc**: Codex dùng chế độ `read-only`; Claude Code dùng chế độ `plan`.
- **Được sửa file**: Codex dùng chế độ `workspace-write`; Claude Code dùng chế độ `acceptEdits`. Đây là lựa chọn mặc định.

Thiết lập quyền được lưu cùng chat. Các nhà cung cấp HTTP không dùng tùy chọn này.

## Cấu hình và lịch sử chat

Cấu hình nằm tại `<config_dir>/ai-hub/config.json`; trên Windows, đường dẫn thường là `%APPDATA%\ai-hub\config.json`. Các cuộc trò chuyện được lưu thành tệp JSON trong `<config_dir>/ai-hub/chats/`; trên Windows là `%APPDATA%\ai-hub\chats\`.

API key được che trong phản hồi cài đặt và không được ghi vào log.


## Thanh chế độ

Nút chế độ cạnh khung nhập tin cho biết nguồn AI, model và mức suy luận hiện tại. Mở nút để đổi nguồn, chọn hoặc nhập model, và chọn mức suy luận khi nguồn hỗ trợ. Lựa chọn mức suy luận được lưu theo từng chat; nguồn, model và mức dùng gần nhất được ghi nhớ cho chat mới trong trình duyệt.

## Tiện ích

Mở **Tiện ích** ở thanh bên để xem trạng thái và quản lý extension. CodeGraph là tiện ích đầu tiên; trang hiển thị trạng thái cài đặt, kết nối Codex/Claude và chỉ mục của thư mục chat đang hoạt động. Mỗi thao tác yêu cầu xác nhận trước khi chạy, ghi đầu ra vào Nhật ký và cho phép dừng tiến trình. Cài đặt CodeGraph cần Node.js và npm.

## Chia việc (phase 1)

AI Hub có thể đề xuất kế hoạch công việc từ nội dung đang soạn hoặc tin nhắn người dùng gần nhất. Mở **Chia việc** ở thanh bên để chỉnh sửa, sắp xếp, thêm hoặc xóa việc và duyệt kế hoạch trước khi thực thi. Kế hoạch được lưu tại `<config_dir>/ai-hub/plans/`. Người lập kế hoạch chọn trong phần Cài đặt; mỗi công việc chỉ chạy khi người dùng bấm **Chạy**, và chỉ bật khi các phụ thuộc đã hoàn tất. Công việc dùng chat riêng theo nhà cung cấp được giao và quyền đọc/ghi theo loại công việc. Không có tự động chạy nối tiếp.
