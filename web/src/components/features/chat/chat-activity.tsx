import { ChevronRight } from "lucide-react";
import { chatText } from "@/text/chat";
import styles from "./chat-message.module.css";

/** One activity entry per supplied log, in the original order; `Command:`/`Lệnh:` lines are real shell commands. */
const COMMAND = /^(?:Command|Lệnh):\s*([\s\S]*)$/i;

/** `provider` stays accepted for compatibility; every provider's logs collapse into the same disclosure. */
export function ChatActivity({ logs }: { logs: string[]; provider?: string }) {
  if (!logs.length) return null;
  const entries = logs.map(text => ({ text, command: COMMAND.exec(text)?.[1] }));
  const commands = entries.filter(entry => entry.command !== undefined).length;
  return <details className={styles.activity}>
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
  </details>;
}
