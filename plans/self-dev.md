# Tự phát triển (Self-dev) — chạy AI Hub từ mã nguồn và dùng chính nó để sửa nó

Trạng thái: đang làm · Bắt đầu: 2026-11-10 · Giám sát: Claude · Code: Codex (gpt-6-luna)

## Yêu cầu của người dùng (đã chốt 2026-11-10)
- KHÔNG dùng file exe release, KHÔNG có "bản thử" chạy song song, KHÔNG thay exe.
- App chạy **local từ mã nguồn** (chế độ dev) và AI sửa **chính bản đang chạy đó**; sửa xong thấy ngay.
- Khi ổn thì người dùng tự build exe sau (có nút, nhưng chỉ khi người dùng bấm).
- (Thiết kế cũ "build bản thử cổng 7879 + thay exe" đã bị huỷ; code dở đã bỏ.)

## Cách chạy chế độ dev
`.\dev.ps1` trong repo (hoặc `dev.cmd`):
1. `cargo build` (debug, target\debug).
2. Chép `target\debug\ai-hub.exe` → `target\dev-run\ai-hub.exe` rồi chạy bản chép đó với `--dev`
   (chạy bản chép để `cargo build` sau này không bị Windows khoá file).
3. Khi app thoát với mã **75** ("xin khởi động lại") → quay lại bước 1 (build lỗi thì giữ bản cũ, chạy lại bản cũ và
   báo lỗi build). Mã khác → dừng. Ctrl+C dừng hẳn.

## Trong app khi `--dev`
- **Giao diện đọc thẳng từ đĩa** (`<repo>\ui\*`) mỗi request thay vì bản nhúng → AI sửa HTML/CSS/JS là có hiệu lực.
  **Tự tải lại trang**: server theo dõi mtime `ui\*` (poll 1s), báo qua SSE `/api/dev/events` → trình duyệt reload.
- **Sửa Rust**: server theo dõi mtime `src\**\*.rs`, `Cargo.toml`. Có thay đổi → thanh thông báo
  "Code Rust đã thay đổi · [Build & khởi động lại]". Bấm → app chạy `cargo build` (target\debug, không bị khoá vì app
  đang chạy bản chép) với log trực tiếp; build lỗi → hiện lỗi, app vẫn chạy bản cũ; build đạt → app thoát mã 75,
  dev.ps1 khởi động bản mới, trình duyệt tự kết nối lại và tải lại.
  Tuỳ chọn "Tự build & khởi động lại khi code Rust đổi" (mặc định tắt).
- **Không khởi động lại khi đang có chat/task AI chạy** → chờ xong (hiện "Đang chờ N việc xong…") hoặc người dùng huỷ.
- Dùng **cùng dữ liệu thật** (%APPDATA%\ai-hub) — đúng ý "sửa chính bản đang chạy".
- Huy hiệu "DEV" ở thanh bên.

## Trang "Tự phát triển" (thanh bên, chỉ khi `--dev`)
- Repo: đường dẫn, nhánh, số file thay đổi, commit cuối.
- "Sửa app bằng AI": mở chat mới / Chia việc với Thư mục = repo (MutiS nếu bật, không thì Codex CLI, quyền Được sửa file).
- Git: "Tạo checkpoint" (commit WIP), "Xem thay đổi" (diff), "Bỏ thay đổi về checkpoint" (xác nhận), "Commit" (tác giả theo
  git config repo — HCodi24).
- Kiểm tra: "Chạy test" (`cargo test` với CARGO_TARGET_DIR=target\dev-check để không đụng target\debug) + cổng kiểm tra MutiS.
- "Build exe release" — chỉ khi người dùng bấm (`cargo build --release`), báo đường dẫn exe.

## Các bước
- [x] 11a — `--dev`: UI từ đĩa + tự tải lại; theo dõi Rust; endpoint build & restart (mã 75) có khoá khi đang chạy việc;
  `dev.ps1`/`dev.cmd`; huy hiệu DEV.
- [x] 11b — trang Tự phát triển: repo/git status, mở chat sửa app, checkpoint/diff/bỏ/commit, chạy test, build release.

## Nhật ký tiến độ
- 2026-11-10: bỏ thiết kế cũ (exe thử + thay exe) theo yêu cầu; viết lại thiết kế chế độ dev.
- 2026-11-10: Hoàn tất 11a: `--dev`/`AI_HUB_DEV`, repo discovery/`AI_HUB_REPO`, UI disk + no-store, watcher/SSE, build/restart code 75, guard việc đang chạy, auto rebuild, badge/banner, launcher scripts, README và unit tests. Offline release build, clippy, 39 tests, JS syntax và diff check đạt.
- 2026-11-10: Hoàn tất 11b: trang dev chỉ hiện trong dev mode; repo refresh 10 giây; mở chat/plan trên repo; API Git cố định cho checkpoint/diff/reset/commit (reset chỉ clean `src` và `ui`, commit/reset bị chặn khi AI jobs chạy); test qua `target/dev-check` kèm MutiS verify khi bật; build release theo nút, SSE logs và đường dẫn exe. Thêm test argv Git và whitelist. Đã cập nhật README. Xác minh: `CARGO_TARGET_DIR=target-codex cargo build --release --offline`, clippy `-D warnings`, 41 tests offline, JS syntax, UTF-8/mojibake scan đều đạt; `git -c core.whitespace=cr-at-eol diff --check` sạch do repo giữ CRLF.
- 2026-11-10: Review 11b (Claude, chạy thật trên bản sao repo): repo status, diff, bỏ thay đổi (giữ .tasks), commit, checkpoint, chạy test, trang hiển thị đều đạt. Claude sửa: git_text cắt mất ký tự đầu tên file (trim đầu dòng porcelain) và danh sách "đã bỏ" liệt kê cả file không bị xoá.


## Step 11c — Chat phát triển (2026-11-10)
- Added the dedicated dev chat button and shared open function with the Self-dev page. Dev chats persist dev_chat, use the repository folder while dev mode is active, and retain their saved folder when dev mode is off.
- Added read-only folder display, ⚒ chat list marker, dev prompt preamble for first messages and plan tasks, MutiS/Codex source selection, and the folder override unit test.
- Verification: `CARGO_TARGET_DIR=target-codex cargo test --offline` (42 passed), `cargo clippy --offline --all-targets -- -D warnings`, `node --check ui/app.js`, `git -c core.whitespace=cr-at-eol diff --check`, and mojibake scan passed. Release build skipped per session instruction. No live AI calls.
- 2026-11-10: 11c "Chat phát triển" (Codex) + review Claude: chạy thật — nút mở chat khoá thư mục repo, server ép thư mục (gửi C:\Windows vẫn chạy ở C:\TOOLi-hub), Codex thấy preamble. Claude sửa: đường dẫn verbatim `\?\C:\...` từ canonicalize (cmd.exe không dùng được làm thư mục làm việc) → strip_verbatim ở server, main (AI_HUB_REPO) và cli_support.
