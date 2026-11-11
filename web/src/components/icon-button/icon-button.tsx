import { Children, cloneElement, isValidElement } from "react";
import { Button, type ButtonProps } from "@/components/button/button";
import { cn } from "@/lib/utils";
import styles from "./icon-button.module.css";
export function IconButton({ className, children, ...props }: ButtonProps & { "aria-label": string }) {
  return <Button variant="ghost" title={props["aria-label"]} {...props} className={cn(styles.button, className)}>
    {Children.map(children, child => isValidElement<{ "aria-hidden"?: boolean }>(child) ? cloneElement(child, { "aria-hidden": true }) : child)}
  </Button>;
}
