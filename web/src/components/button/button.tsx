import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";
import styles from "./button.module.css";
export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
};
export function Button({ className, variant = "secondary", size = "md", ...props }: ButtonProps) {
  return <button type="button" className={cn(styles.button, styles[variant], styles[size], className)} {...props} />;
}
