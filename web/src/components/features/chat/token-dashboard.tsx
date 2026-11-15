import { useMemo } from "react";
import { chatText } from "@/text/chat";
import { chatTokenUsage, formatTokens } from "@/lib/chat-token-usage";
import { useChatStore } from "./chat-store";
import styles from "./token-dashboard.module.css";

/** Recorded reply token reports of the current chat, grouped by the model stored on each reply. */
export function TokenDashboardView({ messages }: { messages: Parameters<typeof chatTokenUsage>[0] }) {
  const usage = useMemo(() => chatTokenUsage(messages), [messages]);
  const t = chatText.tokenDashboard;
  const max = usage.models.reduce((top, group) => Math.max(top, group.total), 0);
  return <section className={styles.dashboard} aria-label={t.title}>
    <h2 className={styles.title}>{t.title}</h2>
    {usage.reports === 0 ? <p className={styles.empty}>{t.empty}</p> : <>
      <p className={styles.total}>{t.total(formatTokens(usage.input), formatTokens(usage.output))}</p>
      <ul className={styles.legend} aria-hidden="true">
        <li><span className={styles.swatch} data-kind="input" />{t.input}</li>
        <li><span className={styles.swatch} data-kind="output" />{t.output}</li>
      </ul>
      <ul className={styles.models}>
        {usage.models.map(group => {
          const name = group.model || t.unknownModel;
          const inputWidth = max ? (group.input / max) * 100 : 0;
          const outputWidth = max ? (group.output / max) * 100 : 0;
          return <li key={group.model || "unknown"} className={styles.model}>
            <span className={styles.name} title={name}>{name}</span>
            <span className={styles.bar} role="img" aria-label={t.modelRow(name, formatTokens(group.input), formatTokens(group.output))}>
              <span className={styles.segment} data-kind="input" style={{ width: `${inputWidth}%` }} />
              <span className={styles.segment} data-kind="output" style={{ width: `${outputWidth}%` }} />
            </span>
            <span className={styles.counts}>
              <span data-kind="input">{`${t.input} ${formatTokens(group.input)}`}</span>
              <span data-kind="output">{`${t.output} ${formatTokens(group.output)}`}</span>
            </span>
          </li>;
        })}
      </ul>
    </>}
    <p className={styles.note}>{t.note}</p>
  </section>;
}

export function TokenDashboard() {
  return <TokenDashboardView messages={useChatStore(state => state.messages)} />;
}
