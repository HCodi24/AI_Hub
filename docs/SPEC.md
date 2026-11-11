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
- `GET /app`, `/app/`, `/app/{*path}`: React 19 / Next.js 16 Pages Router static export trong `web/out`. Route thư mục đọc `index.html`; đường dẫn lạ trả `404.html` hoặc `index.html`. MIME JS/CSS/WOFF2/SVG/JSON/TXT chính xác; chặn traversal và symlink ra ngoài export.
- `build.rs` nhúng `web/out` khi có lúc build; thiếu thư mục tạo bảng rỗng, backend vẫn chạy. Dev ưu tiên repo/web/out; bản thường hỗ trợ cwd/web/out và exe-dir/web/out, rồi tài nguyên nhúng. Chưa có export trả trang tiếng Việt hướng dẫn `npm run build` trong web.
- Build: `cd web`, `npm run typecheck`, `npm run lint`, `npm run build`; sau đó build Rust offline với `CARGO_TARGET_DIR=target/codex`. `/` vẫn là legacy UI, thêm link beta trong sidebar.
- R1: sidebar lịch sử, chọn nguồn/model/quyền/effort, thư mục theo chat, composer Enter/Shift+Enter/Dừng, SSE store Zustand, virtualized messages, markdown sanitize/highlight/copy, hoạt động/token/diff/lỗi. Card plan/debate là link về `/`. Query data dùng React Query; cấu trúc theo `docs/FE_STRUCTURE.md`.
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
- `GET /api/providers/{id}/models`; `POST /api/providers/{id}/test`
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

Chia việc và Tranh luận chỉ có thẻ trong chat, không có `#plans-view` hay nút trang Chia việc.
Mọi điểm vào (composer, gợi ý yêu cầu lớn, MutiS auto_plan, nút dev) dùng chat hiện tại,
tạo chat nếu cần. Planner provider/model giữ trong Cài đặt › Chia việc; MutiS ưu tiên lead.

`POST /api/plans` nhận `chat_id: Option<String>` (UUID; sai trả 400 trước khi dò nguồn),
`append_request: bool` mặc định true và `after_debate_id` tùy chọn để đặt thẻ sau tranh luận.
Khi có chat_id hợp lệ, tạo chat nếu chưa có; thêm user nếu append_request và assistant có
`plan_id`, content rỗng. SSE `planning` trả kế hoạch có ID sớm; `log`, `plan`, `error` giữ nguyên.
Plan lưu `chat_id`, `planning`, `error` với serde defaults cho dữ liệu cũ. ChatMessage có
`plan_id: Option<String>` (default None, bỏ khi serialize None), tương tự debate_id.
Khi planner kết thúc và mỗi lần PUT kế hoạch, assistant được cập nhật bằng markdown tên kế hoạch,
task + assignee + trạng thái (tối đa 1.500 ký tự Unicode). Đọc lại chat trước khi cập nhật để giữ
các tin khác. Lịch sử provider chỉ dùng content tóm tắt, bỏ metadata/output của thẻ và assistant rỗng.
Các lượt chat sau cung cấp tiến độ này cho Bộ nhớ dự án mà không có lượt gọi AI bổ sung.

`ui/js/plan-card.js` và `ui/styles/plan-card.css` dùng chung cho live/lịch sử, cache theo plan_id.
GET kế hoạch tải trạng thái khi mở lại chat; thẻ live giữ nguyên object mà runner đang cập nhật.
Task mở rộng có sửa chi tiết, source/vai trò MutiS, model, effort, quyền, loại, phụ thuộc, thứ tự,
xóa/thêm; kiểm tra chu trình và lưu qua persistPlan. Output/summary/log/diff/verify/review
hiển thị thu gọn trong task. Duyệt, Chạy tất cả (xác nhận inline), Dừng, Chạy tiếp, xác minh,
review lại và tự chạy tiếp dùng runner hiện có; task vẫn dùng chat_id riêng, không điều hướng.
Sidebar chỉ liệt kê plan/debate chưa có chat_id. Mở legacy plan dùng PUT gắn chat mới;
`POST /api/debates/{id}/chat` `{chat_id}` nhận legacy debate, tạo chat theo title/folder,
lưu thẻ và kế hoạch liên kết nếu có; không chạy AI. Bản ghi đã có chat_id không nằm trong mục cũ.


### Tranh luận (`/api/debates`)
- `POST /api/debates` nhận thêm `chat_id` tùy chọn (UUID; sai trả 400). Khi có, tạo chat nếu thiếu và lưu một tin user cùng một tin assistant có `debate_id`, nội dung ban đầu rỗng. Khi kết thúc, cập nhật nội dung bằng tóm tắt phán quyết và các bước (tối đa 1.500 ký tự), hoặc lỗi/dừng; các lượt chat sau dùng nội dung tóm tắt này, bỏ qua assistant rỗng.
- Chat và lịch sử dùng chung `ui/js/debate-card.js`: một thẻ, phán quyết trên biên bản, tab vòng, hai cột (chuyển bên trên màn hình nhỏ), thời gian và hoạt động trực tiếp. Bản ghi và từng lượt có `elapsed_s` mặc định 0 để đọc dữ liệu cũ. Tin nhắn có `debate_id` tải lại thẻ từ `GET /api/debates/{id}`; thẻ đang chạy được dùng lại trong bộ nhớ. Chia việc dùng thư mục của bản tranh luận.
- `POST /api/debates` `{request, folder}` → SSE: `debate`, `round`, `delta{side,round,text}`, `log`, `turn{side,round,turn}`,
  `judging`, `verdict`, `error`, `done`. Codex (vai trò big nếu là codex_cli) và Claude (vai trò lead nếu là claude_cli) đều phải
  đang kết nối, chỉ đọc file. Vòng 1: hai bên đề xuất song song; vòng 2+: mỗi bên phản biện (tiếp tục phiên CLI của mình) và kết
  bằng khối JSON `{agree, disagreements}`; dừng sớm khi cả hai đồng ý. Trọng tài là một phiên Claude CLI MỚI (không session_id),
  model = `mutis.debate_judge_model` hoặc model của Claude, trả JSON phán quyết. Số vòng `mutis.debate_rounds` (1–3, mặc định 2).
- `GET /api/debates`, `GET /api/debates/{id}` (kèm `plan_request` để đưa sang Chia việc). Lưu ở `<data>/debates/<id>.json`.
- Debate có `plan_id: Option<String>` và `chat_id: Option<String>`, serde default `None` cho dữ liệu cũ.
  `chat_id` lưu khi bắt đầu từ chat. `POST /api/debates/{id}/plan` `{plan_id}` kiểm tra hai UUID (sai → 400),
  tranh luận/kế hoạch tồn tại (thiếu → 404), tranh luận đã `done` và có verdict (chưa đạt → 400), rồi lưu liên kết và trả Debate.
- Sau phán quyết, thẻ chung cho chat/lịch sử có “✦ Chia việc & thực hiện” và “▶ Thực hiện với…”.
  Chia việc dùng `planFromRequest(request, {appendRequest:false, afterDebateId})`,
  hiển thị một thẻ kế hoạch riêng ngay sau thẻ tranh luận, dùng chung editor và xác nhận chạy inline.
  `runWholePlan(plan,onProgress)` dùng runner từng task, tuần tự theo phụ thuộc, bỏ việc xong, dừng khi lỗi, hỗ trợ dừng/chạy tiếp.
  Thẻ tải `GET /api/plans/{plan_id}` khi mở lại; tiến trình Chia việc dựa trên liên kết, Thực hiện dựa trên trạng thái công việc.
  Chạy với một nguồn dùng các điều khiển composer hiện có và `sendMessage({text,direct:true})`, bỏ qua MutiS tự chia việc/tranh luận.
  Từ lịch sử mở `chat_id` đã lưu; dữ liệu không có chat_id tạo chat mới trong thư mục tranh luận.

### Tiện ích
- `GET /api/extensions`
- `POST /api/extensions/{id}/actions/{action}`, `/api/extensions/{id}/stop`

Chat trả SSE gồm `status` (provider, model, effort, role và thời điểm bắt đầu), `heartbeat` (elapsed_s, idle_s và last_activity), cùng các sự kiện nội dung `delta`, `reasoning`, `log`, `diff`, `error` và `done`. Heartbeat chỉ thuộc luồng trực tiếp, không lưu vào lịch sử chat. Chat idle timeout được cấu hình bằng `chat_idle_timeout_minutes` trong `/api/config` (mặc định 10 phút, 0 tắt, giới hạn 120).

Planner và thao tác tiện ích có thể trả SSE để cập nhật tiến trình. Các tuyến khác trả JSON hoặc mã trạng thái HTTP.

## Dữ liệu

Dữ liệu nằm trong thư mục cấu hình hệ điều hành, dưới `ai-hub/` (Windows thường là `%APPDATA%\ai-hub`):

- `config.json`: cấu hình và khóa API đã lưu cục bộ. Trường cấu hình MutiS là `mutis`; bộ giải mã vẫn chấp nhận bí danh `orchestration` để đọc cấu hình cũ.
- `chats/`, `attachments/`, `plans/`: lịch sử chat, ảnh theo chat và kế hoạch.
- `mutis/events.jsonl`, `mutis/diagnoses/`: sự kiện và kết quả chẩn đoán. Lần khởi động đầu có thể chuyển `orchestration/events.jsonl` cũ sang thư mục MutiS; thiếu thư mục cũ là bình thường.
- `backups/`: bản sao cấu hình được tạo bởi các thao tác tiện ích cần sao lưu.

## Bảo mật

- Quyền chat/task: `read` (Chỉ đọc), `write` (Được sửa file, mặc định), `full` (Toàn quyền: sửa file và chạy lệnh trong thư mục).
  `Permission::Full` serialize thành `"full"`; task kế hoạch lưu và truyền nguyên giá trị này tới worker.
- Claude CLI: `read` → `--permission-mode plan`; `write` → `--permission-mode acceptEdits`;
  `full` → `--permission-mode acceptEdits --allowedTools Bash PowerShell` (mỗi tool là một đối số riêng).
  Không dùng `bypassPermissions`. Parser nhận lỗi `tool_result` chứa `requires approval` hoặc `haven't granted`;
  khi quyền khác `full`, ghi một lần mỗi lượt: “Lệnh bị chặn: chọn Quyền › Toàn quyền (chạy lệnh) để cho phép AI chạy lệnh.”
- Codex CLI: `read` dùng `read-only`; `write` và `full` cùng dùng `workspace-write`, đã cho phép chạy lệnh trong workspace.
  OpenAI-compatible agent coi `full` như `write` cho công cụ file; Anthropic và HTTP chat không có công cụ chạy lệnh.
  UI ghi “Nguồn này không chạy lệnh” hoặc vô hiệu hóa lựa chọn Toàn quyền với nguồn không phải CLI.
- Nhật ký token định dạng chi phí USD tối đa 4 chữ số thập phân, bỏ số 0 cuối, cho cả JSON number và numeric string
  (ví dụ `0.5748474000000001` → `$0.5748`).

- Máy chủ chỉ bind `127.0.0.1`; middleware chỉ chấp nhận Host loopback cùng cổng và kiểm tra Origin cho yêu cầu không phải GET.
- API key không được đưa vào URL hoặc log. `GET /api/config` trả key đã che; cập nhật với key trống/đã che giữ nguyên key hiện tại.
- CLI nhận prompt qua stdin. MutiS worker bị giới hạn vào các công cụ file được cung cấp; thao tác tiện ích dùng catalog lệnh cố định và yêu cầu xác nhận ở giao diện.
- Đường dẫn làm việc được kiểm tra phía máy chủ. Chức năng thực thi lệnh AI có quyền theo từng task; xác minh/review dùng quyền chỉ đọc theo cấu hình.

## Chế độ agent của nguồn OpenAI-compatible
- `ProviderConfig.agent_capable` (chỉ `openai_compat`): AI được dùng công cụ file (list/read/edit/write, không chạy lệnh) trong thư mục chat.
- `POST /api/chat` nhận `file_tools: none|read|write` (mặc định `none`, lưu trong bản ghi chat). `read` chỉ cấp `list_dir`/`read_file`;
  `write` cấp đủ 4 công cụ. Nguồn chưa bật `agent_capable` mà đòi công cụ file → 400.
- Chạy qua vòng lặp agent (`providers/deepseek_agent.rs`) với base_url/key/model của chính nguồn đó; giữ sandbox đường dẫn, diff,
  giới hạn bước, retry, heartbeat/watchdog như mọi lượt chat.
- Migration khi nạp cấu hình (`merge_agent_providers`): nguồn kiểu `deepseek_agent` cũ được gộp vào nguồn OpenAI-compatible cùng URL
  (đặt `agent_capable`, giữ key nếu nguồn cha chưa có, ánh xạ lại vai trò MutiS) rồi xoá; không có nguồn cha thì đổi kiểu tại chỗ.
  Id cũ `deepseek-agent-deploy` vẫn được hiểu là nguồn agent-capable (`resolve_provider_id`).
## Bộ nhớ dự án (M1)

Nghiệp vụ nằm trong `src/features/memory.rs`, HTTP trong `src/api/memory.rs`,
giao diện trong `ui/js/memory.js` và `ui/styles/memory.css`. Không có lượt gọi AI bổ sung.
Mỗi thư mục canonical có kho `<data_dir>/memory/<project_key>/`; project key là 16 hex
đầu SHA-256 của đường dẫn đã bỏ verbatim prefix và chuyển chữ thường.

Scope id chỉ nhận `[a-z0-9-]{1,48}`. File markdown có frontmatter `scope`, `title`,
`status`, `updated` UTC, `chats`, `files`, `next`, `evidence` và bốn mục bullet:
Mục tiêu, Đã chốt, Đã làm, Còn mở. Index tự tạo lại cho mọi scope đang hoạt động.
Quyết định/đã làm và bằng chứng được cộng dồn, bỏ trùng; câu hỏi mở được thay thế.
Mỗi danh sách tối đa 30 mục, cả file tối đa `max_chars_per_scope`; loại mục cũ của
đã làm/còn mở trước, quyết định sau. `done` thiếu bằng chứng giữ active và ghi việc
cần bằng chứng. `done` có bằng chứng hoặc `obsolete` chuyển vào `archive/`, không xoá
ngay. Mở kho sẽ xoá archive có thời điểm sửa quá 14 ngày; restore đặt lại active.

Config top-level `memory` có serde defaults: `enabled=true`, `max_scopes_in_prompt=3`
(1..10), `max_chars_per_scope=6000` (1000..20000). GET/PUT config round-trip; PUT bỏ
trường memory giữ nguyên cấu hình hiện tại. Dev chat dùng thư mục repo như chat thường.

Chỉ dẫn cập nhật tiếng Việt được nối vào user message mới nhất của bản sao provider
request. Cuối lượt, lấy block `<memory>` cuối cùng, bỏ mọi block khỏi assistant lưu
trữ; JSON sai được bỏ qua và ghi log `Bộ nhớ: khối <memory> không hợp lệ`. SSE `memory`
trả `{scope,status,action:created|updated|archived}` trước `done`. UI ẩn cả block đang
stream chưa kết thúc và cho phép hoàn tác việc đóng scope.

Context gồm index và nội dung đầy đủ tối đa N active scope mới cập nhật nhất, với
lời nhắc kiểm chứng code/git. CLI chỉ nhận context trong user message cuối khi chưa
có session; HTTP kể cả agent nhận ở user message đầu mỗi lượt. Planner, đề xuất và
trọng tài Tranh luận cũng nhận context. Tắt memory bỏ qua cả đọc lẫn ghi tự động.

API (mọi tuyến yêu cầu `?folder=…`, thư mục không tồn tại trả 400):

- `GET /api/memory` → `{project,index,scopes,archived}`.
- `GET /api/memory/{scope}` → `{text}` (active hoặc archived).
- `PUT /api/memory/{scope}` `{text}` → `{text}`; kiểm tra scope frontmatter, tái tạo index.
- `POST /api/memory/{scope}/archive`, `POST /api/memory/{scope}/restore`.
- `DELETE /api/memory/{scope}`: chỉ xoá scope đã archive.
