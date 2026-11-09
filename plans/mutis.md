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

## Bật / tắt (Cài đặt › Điều phối)
- Công tắc chính "Điều phối thông minh" (tắt = AI Hub chạy như cũ, Chia việc chạy tay như bước 8).
- Công tắc con: Retry API · Codex watchdog + tự resume · DeepSeek làm việc trên file · Cổng kiểm tra ·
  Claude review tự động · Tự tạo task sửa khi review FAIL · Ghi nhật ký & thống kê.
- Tham số: người review (provider+model, mặc định claude_cli → anthropic → codex_cli chỉ đọc), số vòng sửa tối đa (2),
  watchdog phút (5), số lần resume (2), retry (3 lần, backoff 2s/4s/8s), ngưỡng file dài cho DeepSeek (400 dòng).

## Các bước (mỗi bước = 1 lượt Codex, Claude review + chạy thử thật)
- [ ] 10a — config `orchestration` + UI Cài đặt › Điều phối; nhật ký sự kiện jsonl; retry HTTP; Codex watchdog + resume.
- [ ] 10b — DeepSeek agent worker (provider kind `deepseek_agent` dùng base_url/key của một openai_compat); diff; bảng định tuyến.
- [ ] 10c — Cổng kiểm tra + Claude review tự động + vòng sửa trong luồng chạy kế hoạch.
- [ ] 10d — Tự chẩn đoán + trang thống kê; README/SPEC.

## Quyết định đã chốt
- Bỏ nhánh "Codex tự giao DeepSeek"; AI Hub (planner/định tuyến) giao thẳng.
- DeepSeek trong AI Hub không được chạy lệnh (bài học treo trên Windows).
- Mọi file ghi UTF-8; quét mojibake là một phần của verify.
- Commit với tác giả HCodi24 <ntson68199@gmail.com>.

## Bài học (từ phiên Dolores FE + AI Hub)
- `codex exec` phải đóng stdin (AI Hub đã gửi prompt qua stdin rồi đóng — giữ nguyên).
- Codex làm hỏng chữ Việt khi ghi bằng PowerShell; hay báo "xong" khi chưa xong; đôi khi xoá mất tính năng khi viết lại.
- DeepSeek ~8192 token output/lượt: file dài → trả rỗng. Cần chế độ chỉ sửa (bơm sẵn đoạn code).
- API Deploy lỗi 500 / ngắt kết nối, có lần sau khi đã sửa file → luôn chụp diff.
- Claude CLI đi kèm app desktop cần `claude auth login` riêng (máy này chưa đăng nhập) → người review mặc định có fallback.

## Nhật ký tiến độ
(Claude ghi sau mỗi bước)
- 10a (2026-11-09): Đã thêm cấu hình/API điều phối, giao diện cài đặt và định tuyến, nhật ký JSONL task plan, retry HTTP, Codex watchdog và tự tiếp tục theo cụm từ dừng sớm.\n