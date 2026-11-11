import { Loader2 } from "lucide-react";
import { chatText } from "@/text/chat";
import styles from "./spinner.module.css";
export function Spinner() { return <span role="status" className={styles.spinner}><Loader2 size={16} aria-hidden="true" />{chatText.loading}</span>; }
