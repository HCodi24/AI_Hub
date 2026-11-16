import { useCallback, useLayoutEffect, useRef } from "react";
import type { VirtuosoHandle } from "react-virtuoso";

const BOTTOM_THRESHOLD = 4;

/** Keeps a chat pinned through content and viewport resizes, unless the reader scrolls away. */
export function useChatScrollFollow(chatId: string, totalCount: number) {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const scheduleFollowRef = useRef<(() => void) | null>(null);
  const frameRef = useRef<number | null>(null);
  const followingRef = useRef(true);
  const lastChatIdRef = useRef(chatId);
  const activeChatIdRef = useRef(chatId);
  const previousListRef = useRef({ chatId, totalCount });

  activeChatIdRef.current = chatId;
  if (lastChatIdRef.current !== chatId) {
    lastChatIdRef.current = chatId;
    followingRef.current = true;
  }

  useLayoutEffect(() => {
    const previous = previousListRef.current;
    previousListRef.current = { chatId, totalCount };
    if (previous.chatId === chatId && totalCount > previous.totalCount && followingRef.current) {
      scheduleFollowRef.current?.();
    }
  }, [chatId, totalCount]);

  const setScroller = useCallback((element: HTMLElement | null | Window) => {
    cleanupRef.current?.();
    cleanupRef.current = null;
    if (element !== null && !(element instanceof HTMLElement)) return;
    if (!element) return;

    const mountedChatId = chatId;
    let userScrollPending = false;
    let userIntentTimer: number | undefined;
    let scrollbarPointerId: number | null = null;
    let touchStartY: number | null = null;
    let resizeObserver: ResizeObserver | undefined;
    let mutationObserver: MutationObserver | undefined;

    const cancelFrame = () => {
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };

    const scheduleFollow = () => {
      if (!followingRef.current || activeChatIdRef.current !== mountedChatId || frameRef.current !== null) return;
      frameRef.current = requestAnimationFrame(() => {
        frameRef.current = null;
        if (followingRef.current && activeChatIdRef.current === mountedChatId) {
          // Use the measured scroll extent, including the footer. Unlike autoscrollToBottom,
          // this API works with followOutput=false and does not arm a later resize-triggered scroll.
          const bottom = Math.max(0, element.scrollHeight - element.clientHeight);
          if (Math.abs(bottom - element.scrollTop) > 1) {
            virtuosoRef.current?.scrollTo({ top: bottom, behavior: "auto" });
          }
        }
      });
    };
    scheduleFollowRef.current = scheduleFollow;

    const atBottom = () => element.scrollHeight - element.clientHeight - element.scrollTop <= BOTTOM_THRESHOLD;

    const markUserIntent = (leavingBottom = false) => {
      userScrollPending = true;
      // Cancel before the browser performs the user's scroll, rather than racing its scroll event.
      cancelFrame();
      followingRef.current = !(leavingBottom && element.scrollTop > 0) && atBottom();
      if (userIntentTimer !== undefined) window.clearTimeout(userIntentTimer);
      userIntentTimer = window.setTimeout(() => {
        userScrollPending = false;
        userIntentTimer = undefined;
      }, 1200);
    };

    const onScroll = () => {
      if ((!userScrollPending && scrollbarPointerId === null) || activeChatIdRef.current !== mountedChatId) return;
      // Keep the gesture alive through touch inertia and held-key scrolling.
      markUserIntent();
      followingRef.current = atBottom();
      if (followingRef.current) scheduleFollow();
    };

    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey && event.deltaY !== 0) markUserIntent(event.deltaY < 0);
    };

    const onTouchStart = (event: TouchEvent) => {
      touchStartY = event.touches.length === 1 ? event.touches[0].clientY : null;
    };

    const onTouchMove = (event: TouchEvent) => {
      if (touchStartY !== null && event.touches.length === 1 && Math.abs(event.touches[0].clientY - touchStartY) > 3) {
        const nextY = event.touches[0].clientY;
        markUserIntent(nextY > touchStartY);
        touchStartY = nextY;
      }
    };

    const onTouchEnd = () => {
      touchStartY = null;
    };

    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey || !(target instanceof HTMLElement)) return;
      if (target !== element && !element.contains(target)) return;
      if (target.closest('input, textarea, select, button, a, summary, [contenteditable]:not([contenteditable="false"]), [role="button"], [role="slider"], [role="tab"]')) return;
      if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(event.key)) {
        markUserIntent(["ArrowUp", "PageUp", "Home"].includes(event.key) || (event.key === " " && event.shiftKey));
      }
    };

    const onPointerDown = (event: PointerEvent) => {
      if (event.target !== element) return;
      const bounds = element.getBoundingClientRect();
      const localX = event.clientX - bounds.left;
      const localY = event.clientY - bounds.top;
      if (localX >= element.clientWidth || localX <= 16 || localX >= bounds.width - 16 || localY >= element.clientHeight || localY <= 0) {
        scrollbarPointerId = event.pointerId;
        markUserIntent(true);
      }
    };

    const onPointerEnd = (event: PointerEvent) => {
      if (event.pointerId !== scrollbarPointerId) return;
      scrollbarPointerId = null;
      followingRef.current = atBottom();
      if (followingRef.current) scheduleFollow();
    };

    const observeContent = (root: ParentNode) => {
      if (!resizeObserver) return;
      if (root instanceof Element) {
        if (root.matches('[data-item-index], [data-testid="virtuoso-item-list"], [data-chat-virtuoso-footer]')) resizeObserver.observe(root);
        if (root.matches('[data-testid="virtuoso-item-list"]')) {
          for (const child of Array.from(root.children)) resizeObserver.observe(child);
        }
      }
      for (const node of Array.from(root.querySelectorAll('[data-item-index], [data-testid="virtuoso-item-list"], [data-chat-virtuoso-footer]'))) {
        resizeObserver.observe(node);
        if (node instanceof HTMLElement && node.dataset.testid === "virtuoso-item-list") {
          for (const child of Array.from(node.children)) resizeObserver.observe(child);
        }
      }
    };

    element.addEventListener("scroll", onScroll, { passive: true });
    element.addEventListener("wheel", onWheel, { passive: true });
    element.addEventListener("touchstart", onTouchStart, { passive: true });
    element.addEventListener("touchmove", onTouchMove, { passive: true });
    element.addEventListener("touchend", onTouchEnd, { passive: true });
    element.addEventListener("touchcancel", onTouchEnd, { passive: true });
    element.addEventListener("pointerdown", onPointerDown, { passive: true });
    element.addEventListener("keydown", onKeyDown);
    element.ownerDocument.addEventListener("pointerup", onPointerEnd, true);
    element.ownerDocument.addEventListener("pointercancel", onPointerEnd, true);

    if (typeof ResizeObserver !== "undefined") {
      resizeObserver = new ResizeObserver(scheduleFollow);
      resizeObserver.observe(element);
      observeContent(element);
    }
    if (typeof MutationObserver !== "undefined") {
      mutationObserver = new MutationObserver(records => {
        for (const record of records) {
          for (const node of Array.from(record.addedNodes)) {
            if (node instanceof Element) observeContent(node);
          }
          for (const node of Array.from(record.removedNodes)) {
            if (node instanceof Element) {
              resizeObserver?.unobserve(node);
              for (const child of Array.from(node.querySelectorAll("*"))) resizeObserver?.unobserve(child);
            }
          }
          if (record.target instanceof Element && record.target.matches('[data-testid="virtuoso-item-list"]')) {
            for (const node of Array.from(record.addedNodes)) if (node instanceof Element) resizeObserver?.observe(node);
          }
        }
        scheduleFollow();
      });
      mutationObserver.observe(element, { childList: true, subtree: true });
    }

    scheduleFollow();
    cleanupRef.current = () => {
      element.removeEventListener("scroll", onScroll);
      element.removeEventListener("wheel", onWheel);
      element.removeEventListener("touchstart", onTouchStart);
      element.removeEventListener("touchmove", onTouchMove);
      element.removeEventListener("touchend", onTouchEnd);
      element.removeEventListener("touchcancel", onTouchEnd);
      element.removeEventListener("pointerdown", onPointerDown);
      element.removeEventListener("keydown", onKeyDown);
      element.ownerDocument.removeEventListener("pointerup", onPointerEnd, true);
      element.ownerDocument.removeEventListener("pointercancel", onPointerEnd, true);
      resizeObserver?.disconnect();
      mutationObserver?.disconnect();
      if (userIntentTimer !== undefined) window.clearTimeout(userIntentTimer);
      cancelFrame();
      if (scheduleFollowRef.current === scheduleFollow) scheduleFollowRef.current = null;
    };
  }, [chatId]);

  return { virtuosoRef, scrollerRef: setScroller };
}
