import type { ComponentPropsWithRef } from "react";
import { cn } from "@/lib/utils";
import styles from "./switch.module.css";
/** Native checkbox attributes minus the fixed type/role, so refs and events keep working. */
export type SwitchProps = Omit<ComponentPropsWithRef<"input">, "type" | "role">;
/** Shared pill switch: a real checkbox with role="switch", so labels, Space and fieldsets behave natively. */
export function Switch({ className, ...props }: SwitchProps) {
  return <input {...props} type="checkbox" role="switch" className={cn(styles.switch, className)} />;
}
