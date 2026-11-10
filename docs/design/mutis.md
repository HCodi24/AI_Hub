# MutiS — tính năng điều phối Claude ↔ Codex ↔ DeepSeek (thiết kế & file bàn giao)

Trạng thái: bản đầu hoàn tất (10a–10d3), đã chạy thử thật · Cập nhật: 2026-11-09 · Người giám sát: Claude · Người code: Codex (gpt-6-luna)

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
- [x] 10c — Cổng kiểm tra + Claude review tự động + vòng sửa trong luồng chạy kế hoạch.
- [x] 10d — Hoàn tất phạm vi MutiS: worker_rules, verify auto-fix, stats, trang MutiS, tự chẩn đoán SSE, lưu/lịch sử findings, allowlist áp dụng setting có xác nhận, tài liệu và kiểm tra offline.

## Quyết định đã chốt
- Bỏ nhánh "Codex tự giao DeepSeek"; AI Hub (planner/định tuyến) giao thẳng.
- DeepSeek trong AI Hub không được chạy lệnh (bài học treo trên Windows).
- Mọi file ghi UTF-8; quét mojibake là một phần của verify.
- Commit với tác giả HCodi24 <ntson68199@gmail.com>.
- Tên tính năng: **MutiS** (đúng chính tả) — tính năng của người dùng; mọi chữ "Điều phối" trên UI đổi thành MutiS.
- MutiS dùng 3 vai trò (lead/big/small) thay vì chọn theo loại provider.
- Thứ tự làm: 10a → 10b → 10b2 → 10c → 10d; mỗi bước Codex (gpt-6-luna, effort high) code, Claude review + chạy thử thật
  bằng bản build riêng `CARGO_TARGET_DIR=target/review` cổng 7897 (không đụng app người dùng đang chạy).

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
- 10b3 (2026-11-09): Fixed MutiS status display to treat unknown as checking, trigger provider tests on popover open and when MutiS is active at load, and retain the team section after source selection. Fallback notices and lead planning fallback now use only confirmed connected providers in role order; sending a MutiS request blocks with a clear message if no lead source is connected. Config migration now adds the seeded DeepSeek agent before migration, fills small with its first model, and prevents empty role provider IDs when any provider exists. Added fallback-order and migration unit tests. Offline release build, clippy, 26 unit tests, JavaScript syntax check, and UTF-8 scan passed. No live AI calls made.
- 10c (2026-11-09): Fixed the mode popover's outside-click handler to ignore detached event targets so MutiS selection keeps the popover open and scrolls the team controls into view. Added `/api/verify` SSE with custom command and script/Cargo/npm detection, total timeout, process-tree cancellation, ANSI stripping and MOJIBAKE scanning. Added persisted plan task verify/review results, lead-role reviewer fallback, review/fix event logging and rounds, per-plan check controls, task review controls and optional sequential continuation. Added detector, mojibake, JSON extraction and fix-round unit coverage. Offline release build, clippy, 30 unit tests, JavaScript syntax check and encoding scan passed. No live AI calls made; interactive app flow not exercised.
- 10c2 (2026-11-09): Fixed live-test follow-ups: added cached server-side `/api/mutis/roles/resolve` for configured provider status and selected connected fallback; the UI resolves roles before planning, running and reviewing. Review now records SKIPPED when no lead reviewer is available without consuming a fix round; high/medium FAIL reviews create and run a titled fix task. Verification now scans tracked changes plus untracked non-ignored files (up to 500), reports invalid UTF-8 separately, skips large/binary files, and applies editable `verify_ignore` globs to MOJIBAKE only. Added state-transition, invalid-encoding, untracked-file and ignore-pattern tests. Offline release build, clippy, 33 unit tests, JavaScript syntax check, diff check and encoding scan passed. No live AI calls made.
- 10d (2026-11-09, partial): Added default `worker_rules` setting and prepended it to MutiS plan-task prompts; verify failures now create same-assignee `Sửa lỗi kiểm tra:` tasks through the existing bounded fix action and rerun gates; added `/api/mutis/stats?days=` for retained JSONL event aggregates; added masked-key helper coverage; documented the feature and scope. Build release offline, clippy, 34 unit tests and `node --check ui/app.js` passed. Not complete: no MutiS stats page, diagnosis endpoint/SSE, diagnosis persistence/history/apply flow, normalized top errors, or stats/diagnosis/whitelist tests. No live AI calls made.

- 10d follow-up (2026-11-09, incomplete): Added the MutiS sidebar statistics view, provider/task-kind tables, recent-event filters, optional task_kind on event records, normalized top-error aggregation, diagnosis history listing, a setting-change endpoint, and diagnosis JSON parsing coverage. The diagnosis endpoint is currently a local placeholder and does not call the reviewer or produce findings. Still missing reviewer-driven diagnosis/SSE logs, findings cards and apply confirmation, full whitelist behavior, stats fixture and whitelist tests, complete documentation parity, and successful full verification. No live AI calls made.
- 10d3 (2026-11-09): Replaced the diagnosis placeholder with an SSE read-only lead review using the chat provider runner and role-resolution fallback order; sends key-free settings, 30-day stats, and up to 150 failure events with details capped at 300 characters. Added local insufficient-data results, diagnosis shape validation with unknown-key removal, timestamped persistence and `diagnosis` event logging. Added the single setting-change allowlist function and `/api/mutis/apply` returning old/new, including type/range/provider/role checks. The MutiS UI now streams logs, renders severity cards, confirms and applies eligible changes, and lists/open past diagnoses. Updated README/SPEC. Verification passed: release build offline, clippy with warnings denied, 37 offline unit tests, `node --check ui/app.js`, UTF-8/mojibake scan and `git diff --check`. No live AI calls made.

- Review cuối (Claude, 2026-11-09, chạy thật với bản sao cấu hình người dùng):
  - 10b3: vai trò tự chọn nguồn đang kết nối (lead → Codex vì Claude chưa đăng nhập, Anthropic chưa có key).
  - 10c2: Claude sửa lộ API key ở /api/mutis/roles/resolve (đã che, thử bằng key giả trên localhost).
  - 10d: worker_rules làm Codex ghi UTF-8 đúng ngay lần đầu; verify FAIL → 2 vòng sửa → verify ĐẠT → review FAIL bắt được
    yêu cầu thử tự mâu thuẫn. Claude sửa: lệnh verify có ngoặc kép (cmd raw_arg), task sửa giữ ngữ cảnh gốc + "(vòng n)".
  - 10d3: Tự chẩn đoán chạy thật (62s, Codex làm lead) → health "kém", phát hiện verify_fail_rate = 0 dù có lỗi →
    Claude xác minh: sự kiện verify chưa bao giờ được ghi → đã sửa (UI ghi event verify). Danh sách trắng Áp dụng: 3 hợp lệ
    nhận, 4 nguy hiểm/sai bị chặn.
- Còn lại (chưa làm): Claude CLI cần người dùng `auth login`; DeepSeek làm việc chưa chạy với API thật (chưa có key trong
  AI Hub, đã thử bằng API giả lập); popover giữ mở khi chọn MutiS chưa kiểm lại bằng click thật.
