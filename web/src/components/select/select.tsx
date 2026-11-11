import type { SelectHTMLAttributes } from "react";
import styles from "./select.module.css";
export function Select({ label, children, ...props }: SelectHTMLAttributes<HTMLSelectElement> & { label: string }) {
  return <label className={styles.control}><span>{label}</span><select {...props}>{children}</select></label>;
}
