import type { Plan, PlanTask } from "@/schema/plans.schema";
import type { Mutis } from "@/schema/mutis.schema";
export function taskPrompt(plan: Plan, task: PlanTask, config: Mutis) {
  const dev = plan.dev_chat ? [
    `Bạn đang sửa chính ứng dụng AI Hub (repo: ${plan.folder}) đang chạy ở chế độ dev.`,
    "- Giao diện ở ui/ tự tải lại khi lưu; code Rust ở src/ cần người dùng bấm 'Build & khởi động lại'.",
    "- Không chạy `cargo build --release`, không tắt tiến trình ai-hub, không sửa target/.",
    "- Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy `cargo test` với CARGO_TARGET_DIR=target/dev-check nếu cần.",
    "- Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không.",
  ].join("\n") : "";
  return [dev, config.enabled ? config.worker_rules : "", `Kế hoạch: ${plan.title}`, `Công việc: ${task.title}`,
    `Chi tiết: ${task.detail}`, "Tóm tắt công việc phụ thuộc đã hoàn tất:",
    ...task.depends_on.map(id => plan.tasks.find(item => item.id === id)?.summary || ""),
  ].filter(Boolean).join("\n");
}
