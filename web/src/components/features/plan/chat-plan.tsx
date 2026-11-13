import type { ReactNode } from "react";
import { Button } from "@/components/button/button";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { PetAvatar } from "@/components/features/pets/pet-avatar";
import { ChatActivity } from "@/components/features/chat/chat-activity";
import type { Plan } from "@/schema/plans.schema";
import { cardText as t } from "@/text/cards";
import chat from "@/components/features/chat/chat-message.module.css";
import styles from "./plan-card.module.css";
/** Plan with approval off: every task reads like a normal chat reply (avatar, text, activity, tokens). The editor stays behind one disclosure. */
export function ChatPlan({ plan, provider, running, onStop, editor }: { plan: Plan; provider: (id: string) => { name: string; kind: string }; running: boolean; onStop: () => void; editor: ReactNode }) {
  const done = plan.tasks.filter(task => task.status === t.complete).length;
  return <section className={styles.chatPlan} aria-label={t.plan}>
    <p className={styles.chatPlanHead}><strong>{t.plan} · {plan.title}</strong> <small>{done}/{plan.tasks.length}</small>
      {running && <Button variant="danger" size="sm" onClick={onStop}>{t.stop}</Button>}</p>
    {plan.tasks.filter(task => task.status !== t.pending || task.summary || task.log.length).map(task => {
      const source = provider(task.assignee);
      const tokenLine = task.log.filter(line => /^Token:/i.test(line)).at(-1);
      const state = task.status === t.active ? "working" : [t.failed, t.fixing].includes(task.status) ? "failed" : "idle";
      return <article key={task.id} className={chat.message}>
        <div className={chat.identity}><PetAvatar provider={source.kind} state={state} /><strong>{source.name}</strong>
          <span className={chat.petStatus} data-state={state} role="status">· #{task.id} {task.title} · {task.status}</span></div>
        {task.summary && <MarkdownView source={task.summary} />}
        <ChatActivity logs={task.log.filter(line => !/^Token:/i.test(line))} />
        {task.diff && <details><summary>{t.diff}</summary><pre className={styles.chatDiff}>{task.diff}</pre></details>}
        {tokenLine && <div className={chat.tokens}>{tokenLine}</div>}
      </article>;
    })}
    <details><summary>{t.plan}</summary>{editor}</details>
  </section>;
}
