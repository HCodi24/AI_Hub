import type { Plan, PlanTask } from "@/schema/plans.schema";
import type { Mutis } from "@/schema/mutis.schema";
export function taskPrompt(plan: Plan, task: PlanTask, config: Mutis) {
  const dev = plan.dev_chat ? [
    "- Write all rules in English.",
    `You are editing the MutiS app itself (repo: ${plan.folder}), which is running in dev mode.`,
    "- React UI in web/src needs the user to click 'Build giao diện' (the page reloads when the build finishes); Rust code in src/ needs 'Build & khởi động lại'.",
    "- Do not run `cargo build --release`, do not kill the ai-hub process, do not modify target/.",
    "- Write files as UTF-8; keep existing features; run `cargo test` with CARGO_TARGET_DIR=target/dev-check if needed.",
    "- Finally report: which files you changed, and whether a Build & khởi động lại is needed.",
  ].join("\n") : "";
  return [dev, config.enabled ? config.worker_rules : "", `Plan: ${plan.title}`, `Task: ${task.title}`,
    `Details: ${task.detail}`, "Summary of completed dependency tasks:",
    ...task.depends_on.map(id => plan.tasks.find(item => item.id === id)?.summary || ""),
  ].filter(Boolean).join("\n");
}
