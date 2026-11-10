# Tái cấu trúc AI Hub (R1–R4) — thiết kế & file bàn giao

Trạng thái: đang làm · Bắt đầu: 2026-11-10 · Giám sát: Claude · Code: Codex

## Nguyên tắc
- **Không đổi hành vi** app: chỉ di chuyển/tách/gộp code. Mọi bước phải qua `scripts/verify.ps1` + `scripts/smoke.ps1`.
- Bản build phụ đặt **trong `target\`** (`target\codex`, `target\review`, `target\dev-check`), không tạo `target-*` ở gốc.
- Không dùng Chat phát triển để sửa app trong lúc tái cấu trúc.

## Hiện trạng (khảo sát 2026-11-10)
- `src/server.rs` 3.385 dòng, 101 hàm, 44 route (mọi tính năng + tiện ích); 19 dòng > 200 ký tự.
- `ui/app.js` 1.778 dòng, 85 hàm, 64 dòng > 200 ký tự; index.html 25, style.css 19 dòng dồn.
- Trùng: `strip_ansi` (server.rs, verify.rs); tìm CLI ở cả server.rs và providers/cli_support.rs.
- Tàn dư "orchestration" (11 chỗ) + thư mục dữ liệu `%APPDATA%\ai-hub\orchestration`.

## Cấu trúc đích
```
src/main.rs, src/app.rs (AppState + router)
src/api/{mod,chat,providers,plans,mutis,verify,extensions,dev,static_ui}.rs   — chỉ HTTP (parse, gọi feature, trả JSON/SSE)
src/features/{chat,plans,mutis,verify,extensions,dev}/                       — logic nghiệp vụ, không phụ thuộc axum
src/providers/*                                                               — giữ, gom phần tìm CLI về cli_support/discovery.rs
src/core/{config,paths,process,text}.rs                                       — dùng chung
ui/index.html, ui/styles/*.css, ui/js/*.js (ES modules)
scripts/{dev.ps1,dev.cmd,verify.ps1,smoke.ps1}
docs/{SPEC.md,CODEX_RULES.md,design/*,history/tasks/*}
```

## Các bước
- [x] R1a (Claude) — chuyển file: docs/, scripts/, lịch sử .tasks → docs/history/tasks, RULES → docs/CODEX_RULES.md;
  sửa đường dẫn trong dev.ps1 (repo = thư mục cha của scripts/), README, docs.
- [x] R1b (Codex) — verify/smoke scripts, verify_ignore default, MutiS auto-detect, README updated.
- [ ] R2 — tách backend.
- [ ] R3 — tách frontend + nhúng nhiều file UI.
- [ ] R4 — dọn tàn dư, thông báo lỗi tiếng Việt thống nhất, README/SPEC.

## Nhật ký
- 2026-11-10: xoá target-codex/target-test (1,5 GB, không ảnh hưởng). R1a xong.
- 2026-11-10: R1b complete. Watcher continues to monitor ui/, src/, Cargo.toml and Cargo.lock only; docs/ and scripts/ are ignored.
- 2026-11-10: R2a in progress. Moved config, AppState, shared text/path/process foundations, and API error types. Handler extraction and router ownership remain for completion; gates are being run against this intermediate state.

- 2026-11-10: R2b complete. Moved `/api/dev/*` handlers and `/`, `/app.js`, `/style.css` serving (including dev disk serving) from `src/server.rs` into `src/api/dev.rs` and `src/api/static_ui.rs`; moved the dev chat-folder unit test. No intended behavior changes.
