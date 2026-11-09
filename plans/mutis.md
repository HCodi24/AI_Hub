# MutiS — tính năng điều phối Claude ↔ Codex ↔ DeepSeek (thiết kế & file bàn giao)

Trạng thái: đang làm · Cập nhật: 2026-11-09 · Người giám sát: Claude · Người code: Codex (gpt-6-luna)

## Mục tiêu
Đưa quy trình Claude ↔ Codex ↔ DeepSeek (đã thử ở repo Dolores FE và khi làm AI Hub) vào AI Hub thành một
tính năng **bật/tắt được**, gồm các cải tiến đã chốt:
1. Retry/backoff cho API (Deploy hay lỗi 500 / mất kết nối).
2. Codex runner cứng: stdin đóng, watchdog chống treo, tự resume khi dừng sớm (tối đa 2 lần), dọn rác.
3. DeepSeek làm việc thật trên file (agent loop, công cụ read/edit/write giới hạn trong thư mục), luôn trả diff,
   chế độ "chỉ sửa", cảnh báo file dài, KHÔNG chạy lệnh.
4. Bảng định tuyến việc (sửa được): DeepSeek = 1–3 file biết rõ vị trí; Codex = nhiều file/cần test; Claude = thiết kế/review.
5. Cổng kiểm tra (verify) sau mỗi task: tự nhận diện dự án (Cargo / npm / scripts/verify/verify.sh / lệnh tuỳ chỉnh)
   + quét chữ tiếng Việt hỏng; kết quả gọn ~10 dòng.
6. Claude review tự động sau task code: đọc diff + kết quả verify, trả JSON {verdict, issues}; FAIL → tạo task sửa
   giao lại (tối đa N vòng).
7. Nhật ký sự kiện + thống kê chi phí/thời gian/làm lại/lỗi review theo task và theo người làm.
8. **Claude tự chẩn đoán tính năng**: đọc nhật ký sự kiện của chính tính năng điều phối (treo, rỗng, verify lỗi,
   review từ chối, retry) → báo cáo lỗi của tính năng + đề xuất chỉnh cài đặt/định tuyến (người dùng bấm áp dụng).

## Bật / tắt (Cài đặt › MutiS)
- Công tắc chính "Bật MutiS" (tắt = AI Hub chạy như cũ, Chia việc chạy tay như bước 8).
- Công tắc con: Retry API · Codex watchdog + tự resume · DeepSeek làm việc trên file · Cổng kiểm tra ·
  Claude review tự động · Tự tạo task sửa khi review FAIL · Ghi nhật ký & thống kê.
- Tham số: người review (provider+model, mặc định claude_cli → anthropic → codex_cli chỉ đọc), số vòng sửa tối đa (2),
  watchdog phút (5), số lần resume (2), retry (3 lần, backoff 2s/4s/8s), ngưỡng file dài cho DeepSeek (400 dòng).

## Các bước (mỗi bước = 1 lượt Codex, Claude review + chạy thử thật)
- [x] 10a — config + UI bật/tắt; nhật ký sự kiện jsonl; retry HTTP; Codex watchdog + resume. Commit c3c0149.
  Review (Claude, chạy thật): API config + validate OK; event task_start/task_end OK (7,8s · 17.649 token);
  resume lần 1 OK. LỖI: resume lần 2 dựng sai argv ("unexpected argument <thread-id>"); cụm "incomplete" bị bắt cả khi
  chỉ được trích dẫn → giao sửa ở đầu 10b. (Lưu ý test: `task_id` gửi dạng chuỗi.)
- [x] 10b — đổi tên → MutiS (`mutis.rs`, `/api/mutis`, alias `orchestration`); sửa lỗi resume;
  DeepSeek agent worker (kind `deepseek_agent`); diff; định tuyến.
- [x] 10b2 — MutiS 3 vai trò (lead = lập kế hoạch & review, big = làm việc lớn, small = làm việc nhỏ), mỗi vai trò
  provider/model/effort; thanh chế độ hiện 3 model + 3 chấm; popover "✦ MutiS (3 AI)" + Đội MutiS; Gửi ở chế độ
  MutiS = lập kế hoạch. Nhiệm vụ: .tasks/step10b2.md (yêu cầu của người dùng sau khi thấy popover chỉ 1 model).
- [ ] 10c — Cổng kiểm tra + Claude review tự động + vòng sửa trong luồng chạy kế hoạch.
- [ ] 10d — Tự chẩn đoán + trang thống kê; README/SPEC.

## Quyết định đã chốt
- Bỏ nhánh "Codex tự giao DeepSeek"; AI Hub (planner/định tuyến) giao thẳng.
- DeepSeek trong AI Hub không được chạy lệnh (bài học treo trên Windows).
- Mọi file ghi UTF-8; quét mojibake là một phần của verify.
- Commit với tác giả HCodi24 <ntson68199@gmail.com>.
- Tên tính năng: **MutiS** (đúng chính tả) — tính năng của người dùng; mọi chữ "Điều phối" trên UI đổi thành MutiS.
- MutiS dùng 3 vai trò (lead/big/small) thay vì chọn theo loại provider.
- Thứ tự làm: 10a → 10b → 10b2 → 10c → 10d; mỗi bước Codex (gpt-6-luna, effort high) code, Claude review + chạy thử thật
  bằng bản build riêng `CARGO_TARGET_DIR=target-test` cổng 7897 (không đụng app người dùng đang chạy).

## Cách tiếp tục ở phiên mới
Đọc file này + .tasks/RULES.md + .tasks/step<bước kế>.md, chạy:
`codex exec -m gpt-6-luna -c model_reasoning_effort='"high"' -s workspace-write --skip-git-repo-check -C 'C:\TOOL\ai-hub' -o .tasks/<bước>.result.md - < .tasks/<bước>.md`
rồi review: build/test/clippy, quét chữ hỏng, chạy thật trên cổng 7897, commit, cập nhật nhật ký dưới đây.

## Bài học (từ phiên Dolores FE + AI Hub)
- `codex exec` phải đóng stdin (AI Hub đã gửi prompt qua stdin rồi đóng — giữ nguyên).
- Codex làm hỏng chữ Việt khi ghi bằng PowerShell; hay báo "xong" khi chưa xong; đôi khi xoá mất tính năng khi viết lại.
- DeepSeek ~8192 token output/lượt: file dài → trả rỗng. Cần chế độ chỉ sửa (bơm sẵn đoạn code).
- API Deploy lỗi 500 / ngắt kết nối, có lần sau khi đã sửa file → luôn chụp diff.
- Claude CLI đi kèm app desktop cần `claude auth login` riêng (máy này chưa đăng nhập) → người review mặc định có fallback.

## Nhật ký tiến độ 10b
- 2026-11-09: Đổi module/cấu hình MutiS, giữ alias config và endpoint cũ, chuyển event log sang thư mục `mutis`; sửa argv Codex dựng mới theo session và chỉ xét đoạn cuối; thêm provider DeepSeek agent, công cụ file giới hạn, diff SSE, key Deploy server-side, định tuyến theo bảng và loại task `small_edit`.
- Xác minh: build release offline, clippy `-D warnings`, 22 unit test và `node --check ui/app.js` thành công. Chưa chạy AI/HTTP thật theo yêu cầu trong RULES.md.

## Nhật ký tiến độ
(Claude ghi sau mỗi bước)
- 10a (2026-11-09): Đã thêm cấu hình/API điều phối, giao diện cài đặt và định tuyến, nhật ký JSONL task plan, retry HTTP, Codex watchdog và tự tiếp tục theo cụm từ dừng sớm.\n

- 10b2 (2026-11-09): Added MutiS lead/big/small role config with legacy planner/reviewer and routing migration, role-based task assignment, role effort/model propagation, composer source selection and per-chat local source memory, three-role popover/settings controls, and DeepSeek path-validation progress logging. Offline release build, clippy, 23 unit tests, JavaScript syntax check, and UTF-8 scan passed. No live AI calls made.
