'use client';

import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
} from 'react';
import { companionModeText } from '@/text/companion';
import { CompanionArt } from './companion-art';
import { companionReact, useCompanion } from './companion-store';
import { useCompanionMotion } from './use-companion-motion';
import styles from './companion.module.css';

const NARROW_QUERY = '(max-width: 767px)';
const PET_LABEL = 'Linh vật đồng hành — nhấn để chào';
const DIZZY_CLICK_COUNT = 5;
const DIZZY_WINDOW_MS = 3000;
const DRAG_THRESHOLD_PX = 4;

interface DragSession {
  pointerId: number;
  startClientX: number;
  startClientY: number;
  startX: number;
  moved: boolean;
}

export interface CompanionProps {
  className?: string;
}

/**
 * Overlay strip hosting the companion pet above the chat composer.
 * Uses a CSS transform transition for horizontal travel and a fading speech bubble for reactions and modes.
 */
export function Companion({ className }: CompanionProps = {}) {
  const stripRef = useRef<HTMLDivElement>(null);
  const clicksRef = useRef<number[]>([]);
  const dragRef = useRef<DragSession | null>(null);
  const suppressClickRef = useRef(false);
  const [narrow, setNarrow] = useState(false);
  const [isDragging, setIsDragging] = useState(false);

  const reaction = useCompanion((state) => state.reaction);
  const mode = useCompanion((state) => state.mode);
  const { x, facing, pose, setHeld, dragTo, setDragging } = useCompanionMotion(stripRef);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const mq = window.matchMedia(NARROW_QUERY);
    const onChange = () => setNarrow(mq.matches);
    onChange();
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  useEffect(() => {
    if (!narrow) return;
    dragRef.current = null;
    setIsDragging(false);
    setDragging(false);
  }, [narrow, setDragging]);

  if (narrow) return null;

  const stripClass = className ? `${styles.strip} ${className}` : styles.strip;
  const petStyle = { '--x': `${x}px` } as CSSProperties;
  const bubbleText = reaction?.text ?? (mode ? companionModeText[mode] : '');

  const handleActivate = () => {
    const now = Date.now();
    const recent = clicksRef.current.filter((ts) => now - ts <= DIZZY_WINDOW_MS);
    recent.push(now);
    if (recent.length >= DIZZY_CLICK_COUNT) {
      clicksRef.current = [];
      companionReact('dizzy');
      return;
    }
    clicksRef.current = recent;
    companionReact('greet');
  };

  const onClick = () => {
    if (suppressClickRef.current) {
      suppressClickRef.current = false;
      return;
    }
    handleActivate();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    suppressClickRef.current = false;
    handleActivate();
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    suppressClickRef.current = false;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    dragRef.current = {
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      startX: x,
      moved: false,
    };
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    const dx = event.clientX - drag.startClientX;
    const dy = event.clientY - drag.startClientY;

    if (!drag.moved) {
      if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
      drag.moved = true;
      setIsDragging(true);
      setDragging(true);
    }

    dragTo(drag.startX + dx);
  };

  const finishDrag = (event: PointerEvent<HTMLDivElement>, suppressNextClick: boolean) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;

    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    if (drag.moved) {
      if (suppressNextClick) {
        suppressClickRef.current = true;
      }
      setIsDragging(false);
      setDragging(false);
    }
  };

  return (
    <div ref={stripRef} className={stripClass}>
      <div
        className={styles.pet}
        style={petStyle}
        role="button"
        tabIndex={0}
        aria-label={PET_LABEL}
        data-dragging={isDragging ? 'true' : undefined}
        onClick={onClick}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={(event) => finishDrag(event, true)}
        onPointerCancel={(event) => finishDrag(event, false)}
        onLostPointerCapture={(event) => finishDrag(event, false)}
        onPointerEnter={() => setHeld(true)}
        onPointerLeave={() => setHeld(false)}
        onFocus={() => setHeld(true)}
        onBlur={() => setHeld(false)}
      >
        <div
          className={styles.bubble}
          aria-live="polite"
          aria-atomic="true"
          data-open={bubbleText ? 'true' : undefined}
          data-align={x < 72 ? 'start' : undefined}
        >
          {bubbleText}
        </div>
        <CompanionArt pose={pose} facing={facing} />
      </div>
    </div>
  );
}
