import type { ComponentType, ReactNode } from "react";
import { AlertTriangle, ChevronRight, FileEdit, FileText, Info, ShieldCheck, ShieldX, Shrink, Wrench, type LucideProps } from "lucide-react";
import { ActivityIcon } from "@/components/icons/activity-icon";
import { CommandIcon } from "@/components/icons/command-icon";
import { PlugIcon } from "@/components/icons/plug-icon";
import { TokenIcon } from "@/components/icons/token-icon";
import { chatText } from "@/text/chat";
import styles from "../chat-message.module.css";

/** One activity entry per supplied log, in the original order; `Command:`/`Lệnh:`/`Bash:` lines are real shell commands. */
const COMMAND = /^(?:Command|Lệnh|Bash):\s*([\s\S]*)$/i;

const FAILURE = /^(?:MCP .* không dùng được|CodeGraph không dùng được|error:)/i;

const MCP = /^MCP (\S+) › (.+)$/;

/** Picks the icon and one-line label for a log; first matching rule wins. */
function describeLog(text: string): { Icon: ComponentType<LucideProps>; label: ReactNode; danger: boolean; command?: string } {
  const danger = FAILURE.test(text) || /^error/i.test(text);
  const command = COMMAND.exec(text)?.[1];
  const mcp = MCP.exec(text);
  let Icon: ComponentType<LucideProps> = Info;
  let label: ReactNode = text.split(/\r?\n/)[0];

  if (danger || /^Cảnh báo:/.test(text)) Icon = AlertTriangle;
  else if (command !== undefined) {
    Icon = CommandIcon;
    label = <>{chatText.ranCommand} <code>{command || chatText.command}</code></>;
  } else if (mcp) {
    Icon = PlugIcon;
    label = <><strong>{mcp[1]}</strong> › {mcp[2]}</>;
  } else if (/^(?:Read file|Đọc) /.test(text)) Icon = FileText;
  else if (/^Đã sửa /.test(text)) Icon = FileEdit;
  else if (/^Bạn đã cho phép/.test(text)) Icon = ShieldCheck;
  else if (/(?:từ chối|Đã chặn)/i.test(text)) Icon = ShieldX;
  else if (/^Token:/.test(text)) Icon = TokenIcon;
  else if (/nén ngữ cảnh/.test(text)) Icon = Shrink;
  else if (/^Công cụ:/.test(text)) Icon = Wrench;

  return { Icon, label, danger, command };
}

/** `provider` stays accepted for compatibility; `running` shows the latest log beside the count while the reply streams. */
export function ChatActivity({ logs, running }: { logs: string[]; provider?: string; running?: boolean }) {
  const entries = logs.map(text => ({ text, ...describeLog(text) }));
  // A failed MCP start or tool call opens the list by itself so the failure is not hidden.
  const failed = logs.some(text => FAILURE.test(text));
  const latest = running ? entries[entries.length - 1] : undefined;
  return <details className={styles.activity} open={failed || undefined} data-running={running || undefined}>
    <summary className={styles.activitySummary}>
      <ActivityIcon size={20} aria-hidden="true" className={styles.activityLogo} />
      <span>{`${chatText.activity} (${entries.length})`}</span>
      {latest && <span className={styles.activityCurrent}>
        <span aria-hidden="true">·</span>
        <latest.Icon size={16} aria-hidden="true" data-danger={latest.danger} />
        <span className={styles.activityLabel}>{latest.label}</span>
      </span>}
      <ChevronRight size={16} aria-hidden="true" />
    </summary>
    <div className={styles.activityList}>
      {entries.length === 0 && <p className={styles.activityLabel}>{chatText.noActivity}</p>}
      {entries.map(({ text, Icon, label, danger, command }, index) => <details key={index} className={styles.activityEntry}>
        <summary className={styles.activitySummary}>
          <Icon size={16} aria-hidden="true" data-danger={danger} />
          <span className={styles.activityLabel} title={command ?? text}>{label}</span>
          <ChevronRight size={16} aria-hidden="true" />
        </summary>
        <pre className={styles.logs}>{text}</pre>
      </details>)}
    </div>
  </details>;
}
