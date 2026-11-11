import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";
import styles from "./button.module.css";
export function Button({ className, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type="button" className={cn(styles.button, className)} {...props} />;
}
