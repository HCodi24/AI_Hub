import { forwardRef, type HTMLAttributes, type ReactNode, type Ref } from "react";
import { cn } from "@/lib/utils";
import styles from "./scroll-area.module.css";

/** Semantic hosts the shared scroll container can render as. */
export type ScrollAreaHost = "div" | "nav" | "section";

export type ScrollAreaProps = HTMLAttributes<HTMLElement> & {
  /** Semantic element to render; defaults to a plain div. */
  as?: ScrollAreaHost;
  children?: ReactNode;
};

/** Shared themed scrollbar appearance class, composable into other scroll containers. */
export const scrollbarClass = styles.scrollbar;

/**
 * Native scroll container with the shared themed scrollbar.
 * Forwards ref, DOM props, style and events to the rendered host element.
 */
export const ScrollArea = forwardRef<HTMLElement, ScrollAreaProps>(function ScrollArea(
  { as = "div", className, children, ...props },
  ref,
) {
  const Tag = as as "div";
  return <Tag ref={ref as Ref<HTMLDivElement>} className={cn(styles.scrollArea, styles.scrollbar, className)} {...props}>{children}</Tag>;
});
