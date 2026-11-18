const APP_PREFIX_START = "Bạn đang sửa chính ứng dụng MutiS (repo: ";
const APP_PREFIX_END = ") đang chạy ở chế độ dev.";

const DEV_CHAT_INSTRUCTIONS =
  "- Giao diện React ở web/src cần người dùng bấm 'Build giao diện' (trang tự tải lại khi build xong); code Rust ở src/ cần 'Build & khởi động lại'.\n" +
  "- Không chạy `cargo build --release`, không tắt tiến trình ai-hub, không sửa target/.\n" +
  "- Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy `cargo test` với CARGO_TARGET_DIR=target\\dev-check nếu cần.\n" +
  "- Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không.\n";

const CURRENT_RULES =
  "- Write all rules in English.\n" +
  "Write every user-facing word in Vietnamese: commentary, progress explanations, summaries, notes and review verdicts. " +
  "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output " +
  "exactly as they are. This holds even when the task instructions are in English.";

// Older saved chats used the same preamble before the Vietnamese language rule was added.
const LEGACY_RULES = "- Write all rules in English.";

/** Remove only a complete app-injected dev-chat prefix from a render-only copy. */
export function stripDevChatPreamble(content: string): string {
  const firstLineEnd = content.indexOf("\n");
  if (firstLineEnd < 0) return content;

  const firstLine = content.slice(0, firstLineEnd);
  if (!firstLine.startsWith(APP_PREFIX_START) || !firstLine.endsWith(APP_PREFIX_END)) return content;
  if (firstLine.slice(APP_PREFIX_START.length, -APP_PREFIX_END.length).length === 0) return content;

  const body = content.slice(firstLineEnd + 1);
  for (const rules of [CURRENT_RULES, LEGACY_RULES]) {
    const prefix = `${DEV_CHAT_INSTRUCTIONS}${rules}\n\n`;
    if (body.startsWith(prefix)) return body.slice(prefix.length);
  }
  return content;
}
