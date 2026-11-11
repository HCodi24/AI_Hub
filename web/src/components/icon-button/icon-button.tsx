import type { ButtonHTMLAttributes } from "react";
import { Button } from "@/components/button/button";
import styles from "./icon-button.module.css";
export function IconButton(props: ButtonHTMLAttributes<HTMLButtonElement> & { "aria-label": string }) {
  return <Button {...props} className={styles.button} />;
}
