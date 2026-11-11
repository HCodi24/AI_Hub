import { chatText } from "@/text/chat";
import styles from "./skeleton.module.css";
export function Skeleton({ card = false }: { card?: boolean }) {
  return <div className={styles.skeleton} data-card={card} role="status" aria-label={chatText.loading}>
    <span /><span /><span />
  </div>;
}
