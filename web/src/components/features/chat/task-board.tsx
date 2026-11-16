import { ArrowRightCircle, CheckCircle2, Circle, Loader2, MinusCircle, XCircle } from "lucide-react";
import type { TaskRow } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { useChatStore } from "./chat-store";
import styles from "./task-board.module.css";

const ICONS = { waiting: Circle, coding: Loader2, checking: Loader2, fixing: Loader2, done: CheckCircle2, failed: XCircle, takeover: ArrowRightCircle, skipped: MinusCircle, stopped: MinusCircle } as const;
const BUSY = new Set(["coding", "checking", "fixing"]);

/** Live progress of the tasks Codex handed over: who is coding, what is being checked, what is being fixed. */
export function TaskBoardView({ tasks }: { tasks: TaskRow[] }) {
  if (!tasks.length) return null;
  const t = chatText.taskBoard;
  return <section className={styles.board} aria-label={t.title} aria-live="polite">
    <strong className={styles.title}>{t.title}</strong>
    <ul>{tasks.map(task => {
      const Icon = ICONS[task.state as keyof typeof ICONS] ?? Circle;
      const actor = task.actor?.trim() || undefined;
      const checker = task.checker?.trim() || undefined;
      const state = t.states[task.state as keyof typeof t.states] ?? task.state;
      const action = actor && BUSY.has(task.state) ? `${actor} ${state.charAt(0).toLowerCase()}${state.slice(1)}` : state;
      const model = task.model?.trim() || undefined;
      const checkerModel = task.checker_model?.trim() || undefined;
      const modelDetails = [model && `${t.model}: ${model}`, checkerModel && `${t.checkerModel}: ${checkerModel}`].filter(Boolean).join("; ");
      const description = [action, modelDetails, task.attempt > 1 ? t.attempt(task.attempt) : "", task.check ? `${checker || t.check} ${task.check}` : ""].filter(Boolean).join(" · ");
      return <li key={task.title} data-state={task.state}>
        <Icon size={16} aria-hidden="true" className={BUSY.has(task.state) ? "spin" : undefined} />
        <span className={styles.name}>{task.title}</span>
        <small title={modelDetails || undefined} aria-label={description}>{action}{task.attempt > 1 ? ` · ${t.attempt(task.attempt)}` : ""}{task.check ? ` · ${checker || t.check} ${task.check}` : ""}</small>
      </li>;
    })}</ul>
  </section>;
}

export function TaskBoard() {
  return <TaskBoardView tasks={useChatStore(state => state.tasks)} />;
}
