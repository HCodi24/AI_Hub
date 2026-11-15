import { ArrowRightCircle, CheckCircle2, Circle, Loader2, MinusCircle, XCircle } from "lucide-react";
import type { TaskRow } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { useChatStore } from "./chat-store";
import styles from "./task-board.module.css";

const ICONS = { waiting: Circle, coding: Loader2, checking: Loader2, fixing: Loader2, done: CheckCircle2, failed: XCircle, takeover: ArrowRightCircle, skipped: MinusCircle } as const;
const BUSY = new Set(["coding", "checking", "fixing"]);

/** Live progress of the tasks Codex handed over: who is coding, what is being checked, what is being fixed. */
export function TaskBoardView({ tasks }: { tasks: TaskRow[] }) {
  if (!tasks.length) return null;
  const t = chatText.taskBoard;
  return <section className={styles.board} aria-label={t.title} aria-live="polite">
    <strong className={styles.title}>{t.title}</strong>
    <ul>{tasks.map(task => {
      const Icon = ICONS[task.state as keyof typeof ICONS] ?? Circle;
      return <li key={task.title} data-state={task.state}>
        <Icon size={16} aria-hidden="true" className={BUSY.has(task.state) ? "spin" : undefined} />
        <span className={styles.name}>{task.title}</span>
        <small>{t.states[task.state as keyof typeof t.states] ?? task.state}{task.attempt > 1 ? ` · ${t.attempt(task.attempt)}` : ""}{task.check ? ` · Luna ${task.check}` : ""}</small>
      </li>;
    })}</ul>
  </section>;
}

export function TaskBoard() {
  return <TaskBoardView tasks={useChatStore(state => state.tasks)} />;
}
