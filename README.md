# AI Hub

AI Hub là ứng dụng chạy cục bộ trên máy, dùng qua trình duyệt: toàn bộ máy chủ là một tệp thực thi Rust duy nhất (axum + tokio + reqwest), còn giao diện web viết bằng HTML/CSS/JS thuần và được nhúng sẵn trong tệp exe. Ứng dụng chỉ lắng nghe trên 127.0.0.1 (mặc định cổng 7878), tự mở trình duyệt và hiển thị giao diện tiếng Việt theo chủ đề tối.

## Tính năng chính

- Nhiều nguồn AI trong cùng một giao diện: OpenAI-compatible API, Anthropic API, Codex CLI, Claude Code CLI. Mọi nguồn OpenAI-compatible (như Deploy/DeepSeek) có thể bật chế độ agent để đọc/sửa file.
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

Với nguồn OpenAI-compatible, nút "Tải danh sách model" sẽ gọi `/models` của Base URL để lấy danh sách model khả dụng. Cấu hình mẫu cho Deploy: Base URL `https://api.deploy.com/api/v1`, model `deepseek-ai/deepseek-v4.1-flash-1017`.

Hai nguồn CLI tiếp tục cuộc trò chuyện qua cơ chế resume, nên ngữ cảnh được giữ giữa các lượt hỏi đáp.

**Chế độ agent (đọc/sửa file) là một chế độ của cùng một nguồn**, không phải nguồn riêng. Với nguồn OpenAI-compatible, bật "Cho phép AI đọc/sửa file trong thư mục chat" trong Cài đặt; khi đó khung chat có ô **"Công cụ file"**: *Không (chỉ chat)* / *Chỉ đọc file* / *Được sửa file*. Agent chỉ có công cụ liệt kê, đọc, sửa, ghi file trong thư mục chat (không chạy lệnh) và luôn trả diff. Dùng chung key, URL và trạng thái kết nối của nguồn, nên Deploy (DeepSeek) chỉ cần cấu hình một lần. Cấu hình cũ có nguồn "DeepSeek làm việc (Deploy)" được tự gộp vào nguồn Deploy khi khởi động.

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

Bản Claude Code CLI cần đăng nhập riêng. Vào **Cài đặt › Claude Code CLI › "Đăng nhập Claude"**. AI Hub mở một phiên đăng nhập web và hiển thị nút **Mở trang đăng nhập**; bấm nút để mở trang Claude trong trình duyệt, đăng nhập rồi sao chép mã trang web cung cấp vào ô **Mã xác nhận** và bấm **Xác nhận**. Luồng này không mở ứng dụng Claude desktop. AI Hub cũng tự kiểm tra trạng thái đăng nhập mỗi 5 giây khi bảng hướng dẫn đang mở. Bấm **Huỷ** để dừng phiên.

Cách khác, cài qua npm:

```powershell
npm i -g @anthropic-ai/claude-code
```

AI Hub cho Claude Code CLI dùng **thư mục cấu hình riêng** `%APPDATA%\ai-hub\claude-config\` (biến `CLAUDE_CONFIG_DIR`): đăng nhập, cài đặt và lịch sử của Claude trong AI Hub tách khỏi `~\.claude` của app Claude desktop, nên hai bên có thể dùng hai tài khoản khác nhau và đăng xuất bên này không ảnh hưởng bên kia.

## Chat và thanh chế độ

Cột bên trái là danh sách các cuộc trò chuyện đã lưu; chọn một chat để tiếp tục hoặc tạo chat mới. Câu trả lời hiện dần theo kiểu streaming, có nút Dừng để ngắt giữa chừng. Nhấn Enter để gửi, Shift+Enter để xuống dòng. Dùng nút 📎, dán ảnh (Ctrl+V) hoặc kéo ảnh vào khung nhập. Mỗi tin nhắn nhận tối đa 5 ảnh PNG/JPEG/WebP/GIF, mỗi ảnh tối đa 10 MB; có thể gửi chỉ ảnh. Bấm thumbnail trong chat để mở ảnh đầy đủ ở tab mới.

Khi AI đang chạy, thanh trạng thái phía trên khung nhập cho biết nguồn, model, effort, thời gian đã chạy và hoạt động gần nhất. Nếu không nhận được sự kiện trong 45 giây, giao diện cảnh báo; từ 120 giây sẽ gợi ý dừng và gửi lại. **Cài đặt › Chat** có thời gian chờ không hoạt động áp dụng cho mọi nguồn (mặc định 10 phút, 0 để tắt, tối đa 120 phút).

Ảnh được lưu cục bộ trong `attachments/<chat_id>/`. Anthropic và OpenAI-compatible gửi ảnh cho model qua API; Claude CLI nhận đường dẫn ảnh trong prompt; Codex CLI nhận từng ảnh bằng `-i`. DeepSeek agent chưa hỗ trợ ảnh. Các provider API chỉ giữ ảnh lịch sử khi tổng dung lượng dưới 15 MB; Claude/Codex CLI chỉ nhận ảnh ở tin nhắn người dùng mới nhất.

Dưới mỗi câu trả lời có các khối có thể gấp mở:

| Khối | Nội dung |
| --- | --- |
| Đang suy nghĩ | Phần suy luận (reasoning) của model |
| Hoạt động | Lệnh CLI đã chạy, tệp đã đọc/sửa, dòng token "Token: vào X (cache Y), ra Z" |
| Thay đổi | Diff của các tệp bị chỉnh sửa |

Chip chế độ nằm ngay cạnh khung nhập và hiển thị nguồn · model · effort hiện tại. Nhấp vào chip để mở popover, tại đó có thể đổi nguồn, đổi model (mục "Model khác…" để nhập tay) và chọn effort: Mặc định / Thấp / Vừa / Cao / Rất cao / Max.

Effort áp dụng cho Codex CLI, Claude Code CLI (`--effort low|medium|high|xhigh|max`), OpenAI-compatible và Anthropic (extended thinking). Khi MutiS đang bật, popover có thêm nguồn "✦ MutiS (3 AI)" hiển thị ba model theo vai trò kèm ba chấm trạng thái, cùng một mục "Đội MutiS". Nhắn tin ở nguồn MutiS là chat bình thường do vai trò Lập kế hoạch & review trả lời; chỉ khi bấm **✦ Gợi ý chia việc** thì MutiS mới lập kế hoạch và chia việc cho 3 AI.

## Chia việc

Nút "✦ Gợi ý chia việc" nằm cạnh nút Gửi (và tự gợi ý khi yêu cầu có vẻ lớn). Người lập kế hoạch là vai trò lead của MutiS hoặc planner đã cấu hình; nó đề xuất các task gồm người phụ trách, phụ thuộc, loại việc (plan/code/test/docs/review/small_edit) và model/effort riêng cho từng task.

**Tự động chia việc** (Cài đặt › MutiS hoặc công tắc trong popover MutiS): khi bật, mỗi tin gửi ở nguồn MutiS được chia việc ngay như bấm ✦ Gợi ý chia việc, không trả lời kiểu chat thường. Mặc định tắt.

**⚖ Tranh luận**: bật công tắc cạnh "✦ Gợi ý chia việc" để gửi yêu cầu thành một thẻ tranh luận trong chat.
Claude đề xuất, Codex đọc và phản biện; các vòng sau tiếp tục đối đáp (mặc định 2 vòng, dừng sớm khi thống nhất).
Một phiên Claude mới làm trọng tài. Phán quyết nằm trên cùng, kèm các chip mở rộng ý đồng ý, tranh cãi, rủi ro và các bước.
Biên bản chia theo tab vòng, hai cột Codex/Claude; màn hình nhỏ có nút chuyển bên. Nội dung dài thu gọn, khi chạy hiện đuôi nội dung,
hoạt động, token, thời gian và nút Dừng. Thẻ được lưu cùng chat, mở lại hoặc tải lại trang vẫn xem được; lịch sử dùng cùng thẻ.
Bấm "✦ Chia việc từ phán quyết" để lập kế hoạch trong đúng thư mục tranh luận. Kết luận ngắn được lưu để chat tiếp và bộ nhớ có ngữ cảnh.
Không AI nào sửa file trong lúc tranh luận. Cài đặt › MutiS cho chỉnh số vòng (1–3) và model trọng tài.

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
| Làm việc nhỏ (small) | small_edit/docs | Nguồn OpenAI-compatible bật chế độ agent (mặc định Deploy/DeepSeek) |

Nếu nguồn của một vai trò không sẵn sàng, vai trò tự chuyển sang một nguồn đã kết nối. Bảng định tuyến (loại việc → vai trò) có thể sửa trong Cài đặt.

Các tùy chọn của MutiS, mỗi tùy chọn bật/tắt kèm tham số:

| Tùy chọn | Mô tả |
| --- | --- |
| Retry API | Thử lại khi gặp 429/5xx/lỗi kết nối, backoff 2s/4s/8s, tối đa 3 lần |
| Codex watchdog | Dừng riêng Codex trong tác vụ MutiS nếu không có đầu ra trong N phút (mặc định 5) |
| Codex tự tiếp tục | Khi Codex báo dừng sớm ("incomplete", "chưa xong"…), tự chạy tiếp phiên đó (tối đa 2 vòng) |
| Giao việc nhỏ cho nguồn agent | MutiS được giao việc nhỏ cho nguồn ở chế độ agent; giới hạn số bước, cảnh báo với tệp dài hơn 400 dòng |
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
| `%APPDATA%\ai-hub\attachments\` | Ảnh đính kèm, chia theo UUID chat |
| `%APPDATA%\ai-hub\plans\` | Kế hoạch của tính năng Chia việc |
| `%APPDATA%\ai-hub\mutis\` | Sự kiện và kết quả chẩn đoán của MutiS |
| `%APPDATA%\ai-hub\backups\` | Bản sao lưu tệp cấu hình trước khi tiện ích sửa |

API key được che mỗi khi hiển thị và không bao giờ xuất hiện trong URL hay trong log.

Về bảo mật, ứng dụng chỉ bind trên 127.0.0.1; từ chối các header Host/Origin lạ (chống DNS rebinding); prompt của CLI được truyền qua stdin thay vì dòng lệnh; và các biến môi trường Claude của máy chủ được xoá trước khi khởi chạy Claude CLI.

## Cấu trúc mã nguồn

```text
src/           Rust: API, nghiệp vụ, nền tảng dùng chung và nhà cung cấp AI
ui/            Giao diện HTML, CSS và JavaScript ES modules
scripts/       Chạy chế độ dev, kiểm tra tổng hợp và smoke test API
build.rs       Nhúng tài nguyên UI vào binary khi biên dịch
docs/          Đặc tả, quy tắc, thiết kế và lịch sử công việc
```

## Kiểm tra

Chạy `scripts\\verify.ps1` để kiểm tra định dạng/encoding và build; chạy `scripts\\smoke.ps1` để khởi động ứng dụng với dữ liệu tạm và kiểm tra các tuyến API chính.

## Chế độ dev (tự phát triển)

Chạy `scripts\\dev.ps1` từ thư mục gốc repo. Chế độ này khởi chạy ứng dụng với UI được đọc trực tiếp từ thư mục `ui\\`, tự tải lại khi giao diện thay đổi và hỗ trợ build lại khi mã Rust hoặc manifest thay đổi. Dữ liệu ứng dụng vẫn dùng thư mục dữ liệu AI Hub của người dùng.

Trong chế độ dev, trang Tự phát triển hiển thị trạng thái repo, diff, checkpoint, commit, test và build release. Chat phát triển được ghim vào repo AI Hub và dùng đúng thư mục repo làm phạm vi làm việc.

Nếu AI Hub dev đang chạy sẵn, `scripts\dev.ps1` chỉ báo PID và địa chỉ rồi dừng; muốn tắt bản cũ và chạy lại từ đầu thì dùng `.\scripts\dev.ps1 -Restart`.
## Bộ nhớ dự án

Mỗi thư mục chat có bộ nhớ markdown riêng để AI chính tiếp tục công việc ở chat sau,
cả chat một nguồn, MutiS và chat phát triển. Cuối mỗi lượt, AI gửi một khối cập nhật
ẩn trong cùng câu trả lời; ứng dụng không gọi thêm AI. Chi phí dự kiến khoảng
300–800 output tokens mỗi lượt có cập nhật, tùy nội dung và model.

Nhấn **📝 Bộ nhớ** cạnh **⚖ Tranh luận** để xem, sửa, lưu trữ, khôi phục hoặc xoá
scope đã lưu trữ. Trong Cài đặt → **Bộ nhớ dự án**, có thể tắt tính năng, chọn số scope
đưa vào prompt (1–10, mặc định 3) và giới hạn ký tự mỗi scope (1000–20000, mặc định 6000).

Các file nằm ở `<data_dir>/memory/<project_key>/`: mỗi scope là `<scope>.md`,
`index.md` tự tạo lại, `project.txt` ghi thư mục dự án và `archive/` giữ scope đã đóng.
`project_key` là 16 ký tự hex đầu SHA-256 của đường dẫn thư mục canonical, bỏ tiền tố
verbatim Windows và chuyển chữ thường. Không sửa tay `index.md` vì ứng dụng tái tạo nó.
Quyết định và việc đã làm được cộng dồn, bỏ trùng; khi vượt giới hạn sẽ bỏ các mục cũ,
ưu tiên giữ quyết định. AI chỉ đóng `done` khi có bằng chứng; `obsolete` dành cho hướng
đã bỏ. Scope đóng được chuyển vào archive, tự xoá sau 14 ngày tính từ lần lưu trữ;
có thể **Hoàn tác** trên thông báo hoặc **Khôi phục** trong bảng bộ nhớ trước hạn đó.

Với Codex/Claude CLI, ngữ cảnh bộ nhớ chỉ được đưa vào phiên mới hoặc khi đổi nguồn;
với nguồn HTTP, nó đi cùng lịch sử mỗi lượt. Planner và Tranh luận cũng đọc bộ nhớ.
Ghi chú AI có thể cũ hoặc sai: AI được nhắc kiểm chứng với code/git trước khi tin.
