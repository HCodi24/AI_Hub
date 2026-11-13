import { ChevronRight } from "lucide-react";
import { PetAvatar } from "@/components/features/pets/pet-avatar";
import { chatText } from "@/text/chat";
import styles from "./chat-message.module.css";

/** Delegated worker logs: `Giao DeepSeek:` opens a task, `DeepSeek:` is one worker log line (see src/providers/delegate.rs). */
const DEEPSEEK_LOG = /^\s*(?:Giao DeepSeek|DeepSeek):/i;

export function ChatActivity({ logs }: { logs: string[] }) {
  if (!logs.length) return null;
  const deepseek = logs.filter(text => DEEPSEEK_LOG.test(text));
  const entries = logs.filter(text => !DEEPSEEK_LOG.test(text))
    .map(text => ({ text, command: /^(?:Command|Lệnh):\s*([\s\S]*)$/i.exec(text)?.[1] }));
  const commands = entries.filter(entry => entry.command !== undefined).length;
  return <>
    {deepseek.length > 0 && <section className={styles.deepseekActivity} aria-label="DeepSeek">
      <div className={styles.identity}>
        <PetAvatar provider="deepseek" />
        <strong>DeepSeek</strong>
      </div>
      <div className={styles.deepseekLogs}>
        {deepseek.map((text, index) => <div key={index} className={styles.deepseekLog}>{text}</div>)}
      </div>
    </section>}
    {entries.length > 0 && <details className={styles.activity}>
      <summary className={styles.activitySummary}>
        <span>{commands ? chatText.ranCommands(commands) : `${chatText.activity} (${entries.length})`}</span>
        <ChevronRight size={16} aria-hidden="true" />
      </summary>
      <div className={styles.activityList}>
        {entries.map((entry, index) => <details key={index} className={styles.activityEntry}>
          <summary className={styles.activitySummary}>
            <span className={styles.activityLabel} title={entry.command ?? entry.text}>
              {entry.command !== undefined ? <>{chatText.ranCommand} <code>{entry.command || chatText.command}</code></> : entry.text.split(/\r?\n/)[0]}
            </span>
            <ChevronRight size={16} aria-hidden="true" />
          </summary>
          <pre className={styles.logs}>{entry.text}</pre>
        </details>)}
      </div>
    </details>}
  </>;
}
