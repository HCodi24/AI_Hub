# Quy ước cấu trúc backend AI Hub

## Mục tiêu

Các đợt chỉnh cấu trúc tách file Rust lớn thành thư mục có `mod.rs` và các file con theo nhiệm vụ.
Ví dụ, nhóm chat tách kiểu bản ghi, lưu trữ và quyền. Nhóm CLI tách đọc đầu ra, môi trường và kiểm thử.

- Giữ tên mô-đun tại các điểm gọi khi có thể. Dùng `pub use` hoặc `pub(crate) use` để xuất lại các mục mà nơi gọi cần.
- Đặt mã HTTP, nghiệp vụ và tích hợp nhà cung cấp theo vai trò để dễ tìm nơi cần sửa.
- Đưa kiểm thử đơn vị cạnh mô-đun tương ứng để dễ đối chiếu với phần triển khai.

## Cấu trúc theo tầng

| Tầng | Vai trò | Vị trí tiêu biểu |
| --- | --- | --- |
| `src/api/` | Đăng ký tuyến HTTP, nhận yêu cầu, trả JSON hoặc luồng phản hồi | `src/api/router.rs`, `src/api/error.rs`, `src/api/chat/`, `src/api/config/`, `src/api/providers/`, `src/api/terminal/` |
| `src/core/` | Cấu hình, quy tắc và tiện ích dùng chung | `src/core/config/`, `src/core/rule/`, `src/core/fs.rs`, `src/core/paths.rs`, `src/core/process.rs`, `src/core/text.rs`, `src/core/time.rs` |
| `src/features/` | Nghiệp vụ của ứng dụng | `src/features/chat/`, `src/features/debate/`, `src/features/changes/`, `src/features/addons/`, `src/features/verify/`, `src/features/providers/` |
| `src/providers/` | Gọi nhà cung cấp qua API hoặc CLI, chạy công cụ và hỗ trợ điều phối | Xem các nhóm bên dưới |

### Cấu hình và nghiệp vụ

- `src/core/config/` chia cấu hình nhà cung cấp, MCP, MutiS, chuyển đổi dữ liệu và lưu cấu hình thành các mô-đun riêng.
- `src/core/rule/` tập hợp quy tắc, gồm UTF-8, giới hạn thư mục và phân công MutiS.
- `src/features/chat/` chứa kiểu tin nhắn, bản ghi, lưu trữ và quyền chat. `src/features/debate/` chứa kiểu dữ liệu, lời nhắc, phân tích kết quả và lưu trữ tranh luận.
- `src/features/providers/` chứa nghiệp vụ kết nối, đăng nhập theo phương thức, đăng xuất và xử lý quyền. `src/api/providers/login.rs` giữ phần tiếp nhận và trả kết quả qua HTTP.

### Tích hợp nhà cung cấp

| Vị trí | Nội dung |
| --- | --- |
| `src/providers/agent/` | Công cụ thao tác file, chạy shell và phạm vi ghi |
| `src/providers/claude/` | Môi trường và nguồn Claude |
| `src/providers/claude_cli/` | Chạy Claude CLI, đọc đầu ra và thu gọn ngữ cảnh |
| `src/providers/codex/` | Môi trường, phiên chạy, truyền thông điệp và thu gọn ngữ cảnh Codex |
| `src/providers/codex_cli.rs` | Điểm tích hợp Codex CLI |
| `src/providers/antigravity_cli/` | Chạy Antigravity CLI, thư mục dữ liệu, đọc đầu ra và thu gọn ngữ cảnh |
| `src/providers/deepseek_agent/` | Vòng chạy DeepSeek Agent, lịch sử, lời nhắc và số liệu sử dụng |
| `src/providers/openai_compat/`, `src/providers/anthropic/` | Định dạng tin nhắn và xử lý luồng phản hồi của hai nhóm API |
| `src/providers/cli_support/` | Tiện ích CLI dùng chung cho lệnh, môi trường, xác thực và đọc JSONL |
| `src/providers/delegate/`, `src/providers/schedule/` | Phân công, báo cáo kết quả, hàng đợi và các vòng điều phối |
| `src/providers/limits/` | Đọc và lưu đệm thông tin giới hạn sử dụng theo nhà cung cấp |
| `src/providers/mcp.rs` | Xử lý MCP ở tầng nhà cung cấp |

### Luồng phụ thuộc

Luồng chính là `api → features → core / providers`. Tầng HTTP chuyển yêu cầu cho nghiệp vụ. Nghiệp vụ dùng cấu hình và tiện ích của `core`, đồng thời gọi tích hợp từ `providers` khi cần.

Cây hiện tại có các điểm gọi trực tiếp giữa tầng. `src/api/providers/login.rs` gọi tiện ích từ `providers`. `src/providers/claude_cli/mod.rs` dùng phần xử lý quyền trong `features`, còn `src/providers/mod.rs` xuất lại phần dò CLI từ `features`.

## Ánh xạ file cũ sang vị trí mới

Các thay đổi nằm trong sáu commit sau:

| Commit | Phạm vi |
| --- | --- |
| `d4f160d` | Tách cấu hình và quy tắc trong `core` |
| `335d2f8` | Tách các nhóm HTTP và đưa đăng ký tuyến vào `router.rs` |
| `b5bc391` | Tách các nhóm nghiệp vụ trong `features` |
| `566f658` | Tách các nhóm tích hợp và điều phối trong `providers` |
| `a81b623` | Xóa file Anthropic cũ |
| `c4f5b7e` | Chuyển phần xử lý đăng nhập và quyền sang nhóm nghiệp vụ nhà cung cấp |

Đích trong bảng là mô-đun chính hoặc file nhận phần trách nhiệm được ghi ở cột cuối.

| File cũ | Vị trí mới | Commit và phần được tách |
| --- | --- | --- |
| `src/core/config/mcp.rs` | `src/core/config/mcp/mod.rs` | `d4f160d`, tách kiểm thử MCP |
| `src/core/config/provider.rs` | `src/core/config/provider/mod.rs` | `d4f160d`, tách xử lý bí mật và kiểm thử |
| `src/core/config/migrate.rs` | `src/core/config/migrate/mod.rs` | `d4f160d`, chuyển đổi cấu hình theo nhóm |
| `src/api/config.rs` | `src/api/config/mod.rs` | `335d2f8`, tách dữ liệu đầu vào, kiểm tra và áp dụng cấu hình |
| `src/api/folder.rs` | `src/api/folder/pick.rs` | `335d2f8`, phần mở hộp thoại chọn thư mục |
| `src/api/terminal.rs` | `src/api/terminal/mod.rs` | `335d2f8`, tách kiểm thử terminal |
| `src/features/changes.rs` | `src/features/changes/mod.rs` | `b5bc391`, tách kiểm thử theo dõi thay đổi |
| `src/features/chat.rs` | `src/features/chat/mod.rs` | `b5bc391`, tách tin nhắn, bản ghi, lưu trữ và quyền |
| `src/features/addons/github.rs` | `src/features/addons/github/inspect.rs` | `b5bc391`, phần kiểm tra kho GitHub |
| `src/providers/agent/files.rs` | `src/providers/agent/files/mod.rs` | `566f658`, chia định nghĩa công cụ, đường dẫn, sửa file và tính chênh lệch |
| `src/providers/cli_support.rs` | `src/providers/cli_support/mod.rs` | `566f658`, chia hỗ trợ lệnh, môi trường, xác thực và đọc đầu ra |
| `src/providers/deepseek_agent.rs` | `src/providers/deepseek_agent/mod.rs` | `566f658`, tách lịch sử, lời nhắc và số liệu sử dụng |
| `src/providers/openai_compat.rs` | `src/providers/openai_compat/mod.rs` | `566f658`, tách tin nhắn, luồng phản hồi và kiểm thử |
| `src/providers/anthropic.rs` | `src/providers/anthropic/mod.rs` | `566f658` thêm mô-đun mới, `a81b623` xóa file cũ |
| `src/api/providers/claude/auth.rs` | `src/features/providers/cli/claude.rs` | `c4f5b7e`, chuyển kiểm tra trạng thái xác thực và nhập tài khoản Claude CLI |
| `src/providers/discovery/mod.rs` | `src/features/providers/cli/discovery/mod.rs` | `c4f5b7e`, chuyển phần dò CLI |

Hộp thoại chọn thư mục nằm ở `src/api/folder/pick.rs`, còn liệt kê thư mục nằm ở `src/api/folder/list.rs`.
Với Claude, thao tác đăng nhập bằng CLI nằm ở `src/features/providers/login_cli/claude.rs`, dùng phần kiểm tra và nhập tài khoản trong nhóm `cli`.

## Quy ước đặt file

- Một thư mục gom một nhóm chức năng. Đặt tên file con theo nhiệm vụ, như `store.rs`, `parser.rs` hoặc `stream.rs`.
- Ưu tiên dùng `mod.rs` để khai báo mô-đun con và xuất lại các mục cần thiết. `src/api/folder/mod.rs` là ví dụ. Một số đầu mối vẫn giữ triển khai, như xử lý WebSocket trong `src/api/terminal/mod.rs`.
- Khi tách kiểm thử, đặt `tests.rs` cạnh `mod.rs` và khai báo qua `#[cfg(test)]`. Ví dụ `src/core/config/mcp/tests.rs` và `src/providers/openai_compat/tests.rs`.
- Khi chỉ tổ chức lại file, giữ nguyên hành vi và giao diện công khai, gồm chữ ký mà nơi gọi đang dùng và hợp đồng HTTP. Bảng trên ghi nhận vị trí mã qua các commit, kể cả đợt chỉnh đăng nhập.

## Việc còn lại

| Hạng mục | Trạng thái hoặc việc cần làm |
| --- | --- |
| Xóa `src/providers/anthropic.rs` cũ nếu còn | `a81b623` đã xóa file này. Cây hiện tại dùng `src/providers/anthropic/`. |
| Tách MCP ở tầng nhà cung cấp | `src/providers/mcp.rs` vẫn là một file. Mô-đun cấu hình MCP trong `src/core/config/mcp/` đã được tách riêng. |
| Tách vòng lặp các bước của DeepSeek Agent | Vòng `for step in 0..max_steps` vẫn nằm trong `src/providers/deepseek_agent/mod.rs`. |
| Rà soát và dọn phần quyền còn lại | `Permission`, `FileTools`, `Effort` còn ở `src/features/chat/permission.rs` và được dùng trong `src/api/chat/handler/resolve.rs`. `write_scope` còn trong yêu cầu ở `src/providers/mod.rs` và được chuyển vào công cụ agent từ `src/providers/deepseek_agent/mod.rs`. |
