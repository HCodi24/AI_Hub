# AI Hub

AI Hub là ứng dụng chạy cục bộ trên máy, dùng qua trình duyệt: toàn bộ máy chủ là một tệp thực thi Rust duy nhất (axum + tokio + reqwest), còn giao diện web viết bằng HTML/CSS/JS thuần và được nhúng sẵn trong tệp exe. Ứng dụng chỉ lắng nghe trên 127.0.0.1 (mặc định cổng 7878), tự mở trình duyệt và hiển thị giao diện tiếng Việt theo chủ đề tối.

## Tính năng chính

- Nhiều nguồn AI trong cùng một giao diện: OpenAI-compatible API, Anthropic API, Codex CLI, Claude Code CLI và DeepSeek làm việc (Deploy).
- Chat có lưu lịch sử, trả lời dạng streaming, nút Dừng và các khối gấp mở "Đang suy nghĩ", "Hoạt động", "Thay đổi".
- Thanh chế độ ngay cạnh khung nhập: đổi nguồn, model và mức suy luận (effort) chỉ trong vài cú nhấp.
- Kiểm tra kết nối từng nguồn bằng chấm trạng thái xanh/đỏ/xám và thông báo lỗi rõ ràng.
- "Tự tìm trên máy" giúp dò Codex CLI và Claude Code CLI đã cài sẵn, kèm nút đăng nhập Claude ngay trong ứng dụng.
- Chia việc: AI đề xuất kế hoạch thành các task có phụ thuộc, người dùng duyệt rồi chạy từng task.
- MutiS: điều phối Claude ↔ Codex ↔ DeepSeek theo ba vai trò, có cổng kiểm tra, review tự động, tự sửa lỗi và thống kê.
- Tự chẩn đoán MutiS: vai trò lead đọc cấu hình và số liệu để chỉ ra vấn đề, kèm đề xuất thay đổi cấu hình an toàn.
- Tiện ích mở rộng (hiện có CodeGraph) với xác nhận trước mỗi thao tác và sao lưu tệp cấu hình.
- Dữ liệu lưu cục bộ trên máy, API key luôn được che khi hiển thị và không xuất hiện trong URL hay log.

## Cài đặt và chạy

Yêu cầu: Rust đã cài và PowerShell trên Windows. Mở PowerShell tại thư mục dự án và biên dịch bản release:

```powershell
cd C:\TOOL\ai-hub
cargo build --release
```

Sau khi biên dịch xong, chạy trực tiếp tệp thực thi:

```powershell
.\target\release\ai-hub.exe
```

Ứng dụng khởi động máy chủ cục bộ, mở trình duyệt tới giao diện và giữ nguyên cửa sổ terminal. Giữ terminal mở trong suốt thời gian dùng; nhấn Ctrl+C để dừng ứng dụng.

Tùy chọn dòng lệnh và biến môi trường:

| Tùy chọn | Ý nghĩa |
| --- | --- |
| `--port PORT` | Đổi cổng lắng nghe của lần chạy hiện tại |
| `--no-open` | Không tự mở trình duyệt |
| `AI_HUB_PORT` | Đặt cổng mặc định khi không truyền `--port` |
| `AI_HUB_CONFIG_PATH` | Trỏ tới một tệp cấu hình khác; plans/chats/dữ liệu MutiS được lưu cạnh tệp đó |

Nếu lúc build báo "Access is denied", nghĩa là một tiến trình ai-hub.exe cũ vẫn đang chạy và giữ tệp. Dừng nó rồi build lại:

```powershell
Stop-Process -Name ai-hub -Force
```

## Nguồn AI và kết nối

Tất cả nguồn được cấu hình trong mục **Cài đặt** của giao diện.

| Nguồn | Cách hoạt động |
| --- | --- |
| OpenAI-compatible API | Nhập Base URL, API key và danh sách model. Dùng được với Deploy/DeepSeek, OpenAI, … |
| Anthropic API | Kết nối trực tiếp bằng API key |
| Codex CLI | Chạy CLI trên máy trong thư mục đã chọn, quyền "Chỉ đọc" hoặc "Được sửa file" |
| Claude Code CLI | Chạy CLI trên máy trong thư mục đã chọn, quyền "Chỉ đọc" hoặc "Được sửa file" |
| DeepSeek làm việc (Deploy) | DeepSeek sửa tệp trong thư mục (chỉ list/read/edit/write, không chạy lệnh, luôn trả diff) |

Với nguồn OpenAI-compatible, nút "Tải danh sách model" sẽ gọi `/models` của Base URL để lấy danh sách model khả dụng. Cấu hình mẫu cho Deploy: Base URL `https://api.deploy.com/api/v1`, model `deepseek-ai/deepseek-v4.1-flash-1017`.

Hai nguồn CLI tiếp tục cuộc trò chuyện qua cơ chế resume, nên ngữ cảnh được giữ giữa các lượt hỏi đáp.

"DeepSeek làm việc (Deploy)" chỉ dùng được khi MutiS đang bật và tùy chọn "DeepSeek làm việc trên file" đang mở. Nút "Dùng key của Deploy" sao chép API key từ nguồn Deploy ở phía máy chủ.

Mỗi nguồn hiển thị một chấm trạng thái:

| Chấm | Ý nghĩa |
| --- | --- |
| Xanh | Đã kết nối |
| Đỏ | Lỗi |
| Xám | Chưa cấu hình hoặc đang kiểm tra |

Nút "Kiểm tra kết nối" chạy lại phép thử cho nguồn tương ứng; ứng dụng hiện toast khi kết nối thành công hoặc thất bại, kèm thông điệp rõ ràng như "Sai API key", "Sai Base URL", "Chưa nhập API key", "Chưa cài …".

### Tìm CLI trên máy

Nút "Tự tìm trên máy" (cho Codex/Claude CLI) dò các vị trí: PATH, PATH trong registry, `%APPDATA%\npm`, thư mục ứng dụng Codex, và bản Claude Code đi kèm ứng dụng Claude desktop (`%LOCALAPPDATA%\Claude-Data\…\claude-code\<version>\…\claude.exe`). Kết quả có thể gồm nhiều phiên bản để bạn chọn. Ứng dụng không bao giờ tự chạy ứng dụng Claude desktop. Ô "Đường dẫn file chạy" cho phép đặt đường dẫn thủ công.

### Đăng nhập Claude Code CLI

Bản Claude Code đi kèm cần đăng nhập riêng. Vào **Cài đặt › Claude Code CLI › "Đăng nhập Claude"**: ứng dụng mở một cửa sổ chạy `claude auth login`, bạn hoàn tất trong trình duyệt, sau đó ứng dụng kiểm tra lại và hiện ✓. Nếu chưa đăng nhập, trạng thái là "Claude Code chưa đăng nhập".

Cách khác, cài qua npm:

```powershell
npm i -g @anthropic-ai/claude-code
```

## Chat và thanh chế độ

Cột bên trái là danh sách các cuộc trò chuyện đã lưu; chọn một chat để tiếp tục hoặc tạo chat mới. Câu trả lời hiện dần theo kiểu streaming, có nút Dừng để ngắt giữa chừng. Nhấn Enter để gửi, Shift+Enter để xuống dòng.

Dưới mỗi câu trả lời có các khối có thể gấp mở:

| Khối | Nội dung |
| --- | --- |
| Đang suy nghĩ | Phần suy luận (reasoning) của model |
| Hoạt động | Lệnh CLI đã chạy, tệp đã đọc/sửa, dòng token "Token: vào X (cache Y), ra Z" |
| Thay đổi | Diff của các tệp bị chỉnh sửa |

Chip chế độ nằm ngay cạnh khung nhập và hiển thị nguồn · model · effort hiện tại. Nhấp vào chip để mở popover, tại đó có thể đổi nguồn, đổi model (mục "Model khác…" để nhập tay) và chọn effort: Mặc định / Thấp / Vừa / Cao / Rất cao / Max.

Effort áp dụng cho Codex CLI, OpenAI-compatible và Anthropic (extended thinking); Claude CLI không dùng effort. Khi MutiS đang bật, popover có thêm nguồn "✦ MutiS (3 AI)" hiển thị ba model theo vai trò kèm ba chấm trạng thái, cùng một mục "Đội MutiS".

## Chia việc

Nút "✦ Gợi ý chia việc" nằm cạnh nút Gửi (và tự gợi ý khi yêu cầu có vẻ lớn). Người lập kế hoạch là vai trò lead của MutiS hoặc planner đã cấu hình; nó đề xuất các task gồm người phụ trách, phụ thuộc, loại việc (plan/code/test/docs/review/small_edit) và model/effort riêng cho từng task.

Trong lúc lập kế hoạch, giao diện hiện log trực tiếp, thời gian đã trôi qua và nút "Huỷ". Chỉ những nguồn đã kết nối mới được giao việc; nếu phải thay nguồn, dòng cảnh báo "⚠ đã đổi từ …" xuất hiện.

Sau khi đề xuất xong, bạn sửa nội dung, sắp xếp lại thứ tự rồi bấm "Duyệt kế hoạch". Kế hoạch có vòng phụ thuộc sẽ bị chặn. Sau khi duyệt, mỗi task có các nút:

| Nút | Khi nào dùng |
| --- | --- |
| Chạy | Bật khi các phụ thuộc của task đã xong |
| Dừng | Ngắt task đang chạy |
| Chạy lại | Chạy lại task |
| Đánh dấu xong | Đánh dấu hoàn tất thủ công |

Giao diện hiển thị tổng token theo từng task và theo cả kế hoạch. Tùy chọn "Tự chạy tiếp" sẽ chạy task sẵn sàng kế tiếp sau khi một task vượt qua cổng kiểm tra, và dừng ngay ở lần lỗi đầu tiên.

## MutiS

MutiS là tính năng điều phối của riêng AI Hub, kết nối Claude ↔ Codex ↔ DeepSeek. Công tắc tổng "Bật MutiS" nằm trong **Cài đặt › MutiS**; khi tắt, ứng dụng hoạt động như trước đây.

Ba vai trò, mỗi vai trò có nguồn/model/effort riêng:

| Vai trò | Nhiệm vụ | Mặc định |
| --- | --- | --- |
| Lập kế hoạch & review (lead) | Lập kế hoạch, review kết quả, tự chẩn đoán | Nguồn đầu tiên đang kết nối trong: Claude Code CLI → Anthropic → Codex CLI (chỉ đọc) |
| Làm việc lớn (big) | code/test | Codex CLI |
| Làm việc nhỏ (small) | small_edit/docs | DeepSeek làm việc |

Nếu nguồn của một vai trò không sẵn sàng, vai trò tự chuyển sang một nguồn đã kết nối. Bảng định tuyến (loại việc → vai trò) có thể sửa trong Cài đặt.

Các tùy chọn của MutiS, mỗi tùy chọn bật/tắt kèm tham số:

| Tùy chọn | Mô tả |
| --- | --- |
| Retry API | Thử lại khi gặp 429/5xx/lỗi kết nối, backoff 2s/4s/8s, tối đa 3 lần |
| Codex watchdog | Dừng Codex nếu không có phản hồi trong N phút (mặc định 5) |
| Codex tự tiếp tục | Khi Codex báo dừng sớm ("incomplete", "chưa xong"…), tự chạy tiếp phiên đó (tối đa 2 vòng) |
| DeepSeek làm việc trên file | Cho phép DeepSeek sửa tệp, có giới hạn số bước, cảnh báo với tệp dài hơn 400 dòng |
| Cổng kiểm tra | Tự nhận diện `scripts/verify/verify.sh` → Cargo → npm, hoặc dùng lệnh tùy chỉnh; luôn kiểm tra UTF-8 và lỗi "mojibake" trong tệp đã sửa và tệp mới; danh sách bỏ qua mặc định `.tasks/**`, `plans/**`, `**/*.md` |
| Review tự động | Vai trò lead đọc diff và kết quả kiểm tra, trả lời PASS/FAIL kèm vấn đề; bỏ qua khi chưa có reviewer kết nối |
| Tự sửa lỗi | Kiểm tra hoặc review thất bại sẽ tạo task "Sửa lỗi …: <task> (vòng n)" cho chính worker đó, kèm ngữ cảnh task gốc; mặc định tối đa 2 vòng |
| Quy tắc cho AI làm việc | Đoạn văn bản ghép vào đầu mọi prompt của worker; mặc định: ghi tệp bằng UTF-8, không xoá tính năng sẵn có, báo cáo phần chưa làm |
| Nhật ký & thống kê | Ghi sự kiện và tổng hợp số liệu phục vụ trang MutiS |

### Trang MutiS

Trang MutiS trong thanh bên gồm các thẻ tổng quan, bảng thống kê theo từng nguồn và theo từng loại việc (số task, tỉ lệ thành công, thời gian trung bình, token, số lần retry, số lần watchdog kill, số lần resume, tỉ lệ kiểm tra/review thất bại, số vòng sửa lỗi, top 5 lỗi), và danh sách 50 sự kiện gần nhất kèm bộ lọc.

### Tự chẩn đoán

"Tự chẩn đoán" (Claude tự đánh giá MutiS) cho vai trò lead đọc cấu hình MutiS (không có key), số liệu thống kê và tối đa 150 sự kiện lỗi gần nhất, rồi trả về mức sức khoẻ (tốt/trung bình/kém) cùng các finding (vấn đề, bằng chứng, nguyên nhân, mức nghiêm trọng, đề xuất).

Finding có thay đổi cấu hình an toàn sẽ có nút "Áp dụng" kèm xác nhận "Đổi …: cũ → mới?". Chỉ các thiết lập MutiS trong danh sách cho phép và nằm trong khoảng hợp lệ mới được đổi (không bao giờ đổi API key, provider, lệnh kiểm tra hay quy tắc worker). Dưới 5 sự kiện, ứng dụng trả "Chưa đủ dữ liệu để chẩn đoán" mà không gọi AI. Các lần chẩn đoán được lưu lại và có thể mở lại. Ví dụ: hệ thống từng phát hiện kết quả kiểm tra không được ghi log (nay đã sửa).

## Tiện ích

Mục **Tiện ích** ở thanh bên hiển thị các thẻ extension. Thẻ đầu tiên là CodeGraph (đồ thị tri thức mã nguồn cho công cụ AI, https://github.com/colbymchenry/codegraph), với trạng thái: đã cài/phiên bản, đã kết nối Codex, đã kết nối Claude, thư mục hiện tại đã được lập chỉ mục hay chưa.

Các thao tác có sẵn: Cài đặt, Kết nối Codex, Kết nối Claude, Lập chỉ mục thư mục, Nâng cấp, Gỡ. Mỗi thao tác đều hiển thị chính xác lệnh sẽ chạy và những tệp sẽ bị thay đổi, sau đó hỏi xác nhận. Trước khi sửa, `~/.codex/config.toml` và `~/.claude.json` được sao lưu vào `%APPDATA%\ai-hub\backups\`. Trong lúc chạy có "Nhật ký" trực tiếp và nút Dừng.

Danh sách lệnh được cố định trong ứng dụng; trình duyệt không thể gửi lệnh tùy ý xuống máy chủ.

## Dữ liệu và bảo mật

Trên Windows, dữ liệu nằm trong `%APPDATA%\ai-hub\`:

| Đường dẫn | Nội dung |
| --- | --- |
| `%APPDATA%\ai-hub\config.json` | Cấu hình, gồm API key lưu cục bộ |
| `%APPDATA%\ai-hub\chats\` | Lịch sử các cuộc trò chuyện |
| `%APPDATA%\ai-hub\plans\` | Kế hoạch của tính năng Chia việc |
| `%APPDATA%\ai-hub\mutis\` | Sự kiện và kết quả chẩn đoán của MutiS |
| `%APPDATA%\ai-hub\backups\` | Bản sao lưu tệp cấu hình trước khi tiện ích sửa |

API key được che mỗi khi hiển thị và không bao giờ xuất hiện trong URL hay trong log.

Về bảo mật, ứng dụng chỉ bind trên 127.0.0.1; từ chối các header Host/Origin lạ (chống DNS rebinding); prompt của CLI được truyền qua stdin thay vì dòng lệnh; và các biến môi trường Claude của máy chủ được xoá trước khi khởi chạy Claude CLI.

## Cấu trúc mã nguồn

| Tệp / thư mục | Vai trò |
| --- | --- |
| `src/main.rs` | Khởi động, xử lý cổng, mở trình duyệt |
| `src/api/mod.rs` | Router HTTP, nhóm route theo tính năng, kiểm tra Host/Origin |
| `src/api/plans.rs` | API kế hoạch, tạo kế hoạch và SSE planner |
| `src/api/chat.rs`, `src/api/config.rs`, `src/api/providers.rs` | API chat, cấu hình và provider |
| `src/api/mutis.rs`, `src/api/verify.rs`, `src/api/extensions.rs` | API MutiS, kiểm tra và tiện ích |
| `src/api/dev.rs`, `src/api/static_ui.rs` | API tự phát triển và phục vụ giao diện |
| `src/app.rs` | AppState và trạng thái dùng chung |
| `src/core/` | Cấu hình, đường dẫn, tiến trình và xử lý văn bản dùng chung |
| `src/core/config.rs` | Cấu hình, che API key |
| `src/features/chat.rs` | Lịch sử chat |
| `src/features/plans.rs` | Kế hoạch của tính năng Chia việc |
| `src/features/mutis.rs` | Cấu hình MutiS, nhật ký sự kiện, thống kê |
| `src/features/verify.rs` | Cổng kiểm tra |
| `src/features/extensions.rs` | Danh mục tiện ích |
| `src/features/dev.rs` | Development-mode business logic |
| `src/features/` | Business logic for chat, plans, MutiS, verify, extensions and dev |
| `src/providers/openai_compat.rs` | Nguồn OpenAI-compatible |
| `src/providers/anthropic.rs` | Nguồn Anthropic API |
| `src/providers/codex_cli.rs` | Nguồn Codex CLI |
| `src/providers/claude_cli.rs` | Nguồn Claude Code CLI |
| `src/providers/deepseek_agent.rs` | DeepSeek làm việc (Deploy) |
| `src/providers/cli_support.rs` | Phần dùng chung cho các nguồn CLI |
| `src/providers/discovery.rs` | CLI resolution, candidate discovery and version probing |
| `ui/index.html`, `ui/js/*.js`, `ui/styles/*.css` | Giao di?n web ES module, nh?ng trong exe |
| `docs/design/` | Thiết kế MutiS và Tự phát triển, tiến độ, bài học |
| `docs/SPEC.md` | Đặc tả kỹ thuật |
| `docs/CODEX_RULES.md` | Quy tắc khi giao việc cho Codex |
| `docs/history/tasks/` | Lịch sử các nhiệm vụ đã giao |
| `scripts/` | dev.ps1, dev.cmd, verify.ps1 (kiểm tra build/UTF-8), smoke.ps1 (kiểm tra API cục bộ) |

Chạy kiểm tra đầy đủ bằng `.\scripts\verify.ps1`; chạy smoke test API bằng `.\scripts\smoke.ps1`. Có thể chạy bộ kiểm thử Rust riêng bằng:

```powershell
cargo test
```

## Chế độ dev (tự phát triển)

Chạy `.\scripts\dev.ps1` từ thư mục gốc repo (hoặc `scripts\dev.cmd`). App chạy từ bản sao `target\dev-run\ai-hub.exe`, dùng dữ liệu AI Hub thật của bạn. UI được đọc từ `ui\` mỗi request và tự tải lại khi thay đổi. Khi Rust hoặc Cargo manifest đổi, banner cho phép build và khởi động lại; có thể bật tự build trong Cài đặt › Tự phát triển. Build thành công khởi động lại app qua launcher. Dùng `.\scripts\verify.ps1` để chạy kiểm tra tổng hợp và `.\scripts\smoke.ps1` để kiểm tra API với cấu hình tạm.

Trong chế độ dev, nút **Tự phát triển** ở thanh bên hiển thị nhánh, file thay đổi và commit gần nhất; thông tin repo tự làm mới mỗi 10 giây. Có thể mở chat với quyền sửa file hoặc lập kế hoạch trên repo này, tạo checkpoint, xem diff, bỏ thay đổi có xác nhận, commit, chạy test và build exe release theo yêu cầu. Test dùng `target\dev-check` để tách sản phẩm biên dịch khỏi app đang chạy. MutiS verify chạy tiếp sau `cargo test` khi cổng kiểm tra được bật. Không thao tác nào tự push; build release chỉ chạy khi bấm nút. Nếu build báo `Access is denied`, hãy đóng exe release đang chạy để nhả khóa tệp.

Nút **⚒ Chat phát triển** mở một chat thường được ghim vào repo AI Hub. Trong dev mode, server luôn dùng đường dẫn repo dù client gửi folder khác; folder hiển thị chỉ đọc. Khi MutiS bật, chat dùng MutiS; nếu tắt, chat dùng Codex CLI với mức suy luận cao. Chat lưu cờ `dev_chat`, hiện biểu tượng ⚒ trong lịch sử và gửi hướng dẫn tự phát triển cho yêu cầu đầu tiên cũng như các task phát sinh từ kế hoạch. Nếu mở lại chat khi dev mode đã tắt, chat tiếp tục dùng folder đã lưu và báo chế độ dev đang tắt.
