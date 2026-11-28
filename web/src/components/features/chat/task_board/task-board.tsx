import { useId, useState } from "react";
import { ArrowRightCircle, CheckCircle2, ChevronDown, ChevronUp, Circle, Loader2, MinusCircle, XCircle } from "lucide-react";
import type { TaskRow } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { PetAvatar } from "@/components/features/settings/pet/sprite/pet-avatar";
import { usePetId } from "@/components/features/settings/pet/sprite/use-pet-id";
import { kindFromModel } from "@/lib/pet-choice";
import type { PetState } from "@/lib/chat-pet-state";
import { useChatStore } from "../chat-store";
import styles from "./task-board.module.css";

const ICONS = { waiting: Circle, fixwait: Circle, limited: XCircle, coding: Loader2, checking: Loader2, fixing: Loader2, done: CheckCircle2, failed: XCircle, takeover: ArrowRightCircle, skipped: MinusCircle, stopped: MinusCircle } as const;
const BUSY = new Set(["coding", "checking", "fixing"]);
/** Rows that left the workers' hands: the attempt count says nothing there. */
const NO_ATTEMPT = new Set(["takeover", "skipped"]);
/** Pose of the pet beside a task: running (the sheet's last row) for every busy state, coded, checked or fixed. */
const PET_STATE: Record<string, PetState> = { waiting: "waiting", fixwait: "waiting", coding: "coding", checking: "coding", fixing: "coding", failed: "failed" };

/** "Codex · GPT-6 Luna": who works, then the exact model; the model alone when its own name already says who. */
function who(name: string | undefined, model: string | undefined) {
  if (!model) return name;
  return !name || model.toLowerCase().includes(name.toLowerCase()) ? model : `${name} · ${model}`;
}

function TaskItem({ task }: { task: TaskRow }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const t = chatText.taskBoard;
  const Icon = ICONS[task.state as keyof typeof ICONS] ?? Circle;
  const model = task.model?.trim() || undefined;
  const checkerModel = task.checker_model?.trim() || undefined;
  const petModel = (task.state === "checking" ? checkerModel || model : model) ?? "";
  const petProvider = kindFromModel(petModel);
  const hasPet = Boolean(usePetId(petProvider, petModel));
  const actor = hasPet ? undefined : who(task.actor?.trim() || undefined, model);
  const checker = hasPet ? undefined : who(task.checker?.trim() || undefined, checkerModel);
  const state = t.states[task.state as keyof typeof t.states] ?? task.state;
  const action = actor && BUSY.has(task.state) ? `${actor} ${state.charAt(0).toLowerCase()}${state.slice(1)}` : state;
  const check = task.check?.trim();
  const reason = task.reason?.trim();
  const ReasonChevron = open ? ChevronUp : ChevronDown;
  const reasonLabel = open ? t.hideReason : t.showReason;
  const showAction = task.state !== "done" || !check;
  const modelDetails = [model && `${t.model}: ${model}`, checkerModel && `${t.checkerModel}: ${checkerModel}`].filter(Boolean).join(" · ");
  return <li data-state={task.state}>
    <Icon size={16} aria-hidden="true" className={BUSY.has(task.state) ? "spin" : undefined} />
    <span className={styles.name}>{task.title}</span>
    <span className={styles.avatar}>
      <PetAvatar small provider={petProvider} model={petModel} state={PET_STATE[task.state] ?? "idle"} />
    </span>
    <small className={styles.details} title={modelDetails || undefined}>
      {showAction ? (task.state === "failed" ? <strong data-verdict="FAIL">{state}</strong> : action) : <span className="sr-only">{state}{" · "}</span>}
      {check && <>{showAction && " · "}{checker && `${checker} `}<strong data-verdict={check.toUpperCase()}>{task.check}</strong></>}
      {task.attempt > 1 && !NO_ATTEMPT.has(task.state) && <>{" · "}{t.attempt(task.attempt)}</>}
      {modelDetails && <span className="sr-only">{" · "}{modelDetails}</span>}
      {reason && (
        <button
          type="button"
          className={styles.reasonToggle}
          aria-expanded={open}
          aria-controls={id}
          aria-label={reasonLabel}
          title={open ? undefined : reason}
          data-tooltip-title={t.reasonTitle}
          onClick={() => setOpen(previous => !previous)}
        >
          <ReasonChevron size={14} aria-hidden="true" />
        </button>
      )}
    </small>
    {reason && open && <p id={id} className={styles.reason}>{reason}</p>}
  </li>;
}

/** Live progress of the tasks Codex handed over: who is coding, what is being checked, what is being fixed. */
export function TaskBoardView({ tasks }: { tasks: TaskRow[] }) {
  const [collapsed, setCollapsed] = useState(false);
  if (!tasks.length) return null;
  const t = chatText.taskBoard;
  const Chevron = collapsed ? ChevronDown : ChevronUp;
  return <section data-glass="overlay" className={styles.board} aria-label={t.title} aria-live="polite">
    <button type="button" className={styles.toggle} aria-expanded={!collapsed} title={collapsed ? t.expand : t.collapse} onClick={() => setCollapsed(!collapsed)}>
      <strong className={styles.title}>{t.title}</strong>
      {collapsed && <small>{t.progress(tasks.filter(task => task.state === "done").length, tasks.length)}</small>}
      <Chevron size={16} aria-hidden="true" />
    </button>
    {!collapsed && <ul>{tasks.map(task => <TaskItem key={task.title} task={task} />)}</ul>}
  </section>;
}

export function TaskBoard({ enabled }: { enabled: boolean }) {
  const tasks = useChatStore(state => state.tasks);
  const streaming = useChatStore(state => state.streaming);
  return enabled && streaming ? <TaskBoardView tasks={tasks} /> : null;
}
