import { MessagesSquare } from "lucide-react";
import { chatText } from "@/text/chat";
import styles from "./empty-state.module.css";
export function EmptyState() {
  return <div className={styles.empty}><MessagesSquare size={36} aria-hidden="true" /><h1>{chatText.greeting}</h1><p>{chatText.intro}</p></div>;
}
