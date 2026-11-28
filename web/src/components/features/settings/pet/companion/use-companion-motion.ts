'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import type { CompanionPose } from './companion-art';
import { companionReact, useCompanion, type CompanionEvent } from './companion-store';

const PET_WIDTH = 56;
const WALK_SPEED_PX_PER_SEC = 60;
const STEP_MIN_PX = 60;
const STEP_MAX_PX = 220;
const TICK_MIN_MS = 3000;
const TICK_MAX_MS = 6000;
const SLEEP_AFTER_MS = 60_000;
const WELCOME_BACK_AFTER_MS = 10 * 60 * 1000;
const IDLE_CHANCE = 0.35;

const REACTION_POSE: Record<CompanionEvent, CompanionPose> = {
  remembered: 'remember',
  done: 'happy',
  question: 'surprised',
  error: 'sad',
  greet: 'happy',
  sent: 'happy',
  stopped: 'sad',
  newChat: 'happy',
  switchChat: 'surprised',
  deleted: 'sad',
  attached: 'surprised',
  welcomeBack: 'happy',
  refocus: 'happy',
  dizzy: 'surprised',
  themeChanged: 'happy',
};

function randomRange(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export interface CompanionMotion {
  x: number;
  facing: 'left' | 'right';
  pose: CompanionPose;
  setHeld: (v: boolean) => void;
  dragTo: (x: number) => void;
  setDragging: (v: boolean) => void;
}

/**
 * Sparse setTimeout-driven motion state machine for the companion pet.
 * Outputs target `x` px for CSS transitions (no requestAnimationFrame / setInterval),
 * pauses while hidden, blurred, held, in a persistent mode, or under prefers-reduced-motion,
 * and overlays store reactions.
 */
export function useCompanionMotion(bounds: { current: HTMLElement | null }): CompanionMotion {
  const reaction = useCompanion((state) => state.reaction);
  const mode = useCompanion((state) => state.mode);
  const hasReaction = reaction !== null;
  const hasMode = mode !== null;

  const [x, setX] = useState(0);
  const [facing, setFacing] = useState<'left' | 'right'>('right');
  const [basePose, setBasePose] = useState<CompanionPose>('idle');
  const [held, setHeld] = useState(false);
  const [dragging, setDraggingState] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [blurred, setBlurred] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);

  const xRef = useRef(0);
  const lastReactionAtRef = useRef(0);
  const blurredAtRef = useRef<number | null>(null);

  const dragTo = useCallback(
    (nextX: number) => {
      const containerWidth = bounds.current?.clientWidth ?? 0;
      const maxX = Math.max(0, containerWidth - PET_WIDTH);
      const clamped = Math.round(clamp(nextX, 0, maxX));
      if (clamped < xRef.current) {
        setFacing('left');
      } else if (clamped > xRef.current) {
        setFacing('right');
      }
      if (clamped !== xRef.current) {
        xRef.current = clamped;
        setX(clamped);
      }
    },
    [bounds],
  );

  const setDragging = useCallback((v: boolean) => {
    setDraggingState(v);
    if (!v) {
      lastReactionAtRef.current = Date.now();
    }
  }, []);

  // Track visibility, reduced motion, and window focus/blur; clean up listeners on unmount.
  useEffect(() => {
    if (lastReactionAtRef.current === 0) {
      lastReactionAtRef.current = Date.now();
    }

    const onVisibility = () => setHidden(document.hidden);
    onVisibility();
    document.addEventListener('visibilitychange', onVisibility);

    const mq =
      typeof window !== 'undefined' && typeof window.matchMedia === 'function'
        ? window.matchMedia('(prefers-reduced-motion: reduce)')
        : null;
    const onMotionChange = () => setReducedMotion(mq ? mq.matches : false);
    onMotionChange();
    mq?.addEventListener('change', onMotionChange);

    const onBlur = () => {
      if (blurredAtRef.current === null) {
        blurredAtRef.current = Date.now();
      }
      setBlurred(true);
    };

    const onFocus = () => {
      const blurredAt = blurredAtRef.current;
      blurredAtRef.current = null;
      setBlurred(false);
      const awayMs = blurredAt !== null ? Date.now() - blurredAt : 0;
      companionReact(awayMs > WELCOME_BACK_AFTER_MS ? 'welcomeBack' : 'refocus');
    };

    if (typeof window !== 'undefined') {
      window.addEventListener('blur', onBlur);
      window.addEventListener('focus', onFocus);
    }

    return () => {
      document.removeEventListener('visibilitychange', onVisibility);
      mq?.removeEventListener('change', onMotionChange);
      if (typeof window !== 'undefined') {
        window.removeEventListener('blur', onBlur);
        window.removeEventListener('focus', onFocus);
      }
    };
  }, []);

  // Clamp x immediately whenever the strip shrinks so position and bubble alignment stay in sync.
  useEffect(() => {
    const el = bounds.current;
    if (!el || typeof ResizeObserver === 'undefined') return;

    const clampToBounds = () => {
      const maxX = Math.max(0, el.clientWidth - PET_WIDTH);
      const clamped = clamp(xRef.current, 0, maxX);
      if (clamped !== xRef.current) {
        xRef.current = clamped;
        setX(clamped);
      }
    };

    clampToBounds();
    const observer = new ResizeObserver(clampToBounds);
    observer.observe(el);

    return () => {
      observer.disconnect();
    };
  }, [bounds]);

  // Reset inactivity sleep timer whenever a new reaction arrives.
  useEffect(() => {
    if (reaction !== null) {
      lastReactionAtRef.current = Date.now();
    }
  }, [reaction]);

  // Face right once when typing starts; reset inactivity timer when persistent mode clears.
  useEffect(() => {
    if (mode === 'typing') {
      setFacing('right');
    }
    if (mode === null) {
      lastReactionAtRef.current = Date.now();
    }
  }, [mode]);

  // Autonomous idle / walk / sleep loop paused during drag, reactions, mode, hold, blur, hidden tab, or reduced motion.
  useEffect(() => {
    if (hidden || blurred || reducedMotion || held || dragging || hasReaction || hasMode) {
      setBasePose('idle');
      return;
    }

    let cancelled = false;
    let timerId: ReturnType<typeof setTimeout> | null = null;

    const scheduleDecision = () => {
      timerId = setTimeout(decideNextAction, Math.round(randomRange(TICK_MIN_MS, TICK_MAX_MS)));
    };

    const decideNextAction = () => {
      if (cancelled) return;

      // Fall asleep after ~60 s without any reaction.
      if (Date.now() - lastReactionAtRef.current >= SLEEP_AFTER_MS) {
        setBasePose('sleep');
        return;
      }

      const containerWidth = bounds.current?.clientWidth ?? 0;
      const maxX = Math.max(0, containerWidth - PET_WIDTH);

      if (maxX <= 0 || Math.random() < IDLE_CHANCE) {
        setBasePose('idle');
        scheduleDecision();
        return;
      }

      const currentX = clamp(xRef.current, 0, maxX);
      const step = randomRange(STEP_MIN_PX, STEP_MAX_PX);
      const dir = Math.random() < 0.5 ? -1 : 1;

      let targetX = Math.round(clamp(currentX + dir * step, 0, maxX));
      // If clamped near an edge, try walking the opposite way instead.
      if (Math.abs(targetX - currentX) < 12) {
        targetX = Math.round(clamp(currentX - dir * step, 0, maxX));
      }

      const distance = Math.abs(targetX - currentX);
      if (distance < 1) {
        setBasePose('idle');
        scheduleDecision();
        return;
      }

      xRef.current = targetX;
      setFacing(targetX < currentX ? 'left' : 'right');
      setX(targetX);
      setBasePose('walk');

      const walkDurationMs = Math.round((distance / WALK_SPEED_PX_PER_SEC) * 1000);
      timerId = setTimeout(() => {
        if (cancelled) return;
        setBasePose('idle');
        scheduleDecision();
      }, walkDurationMs);
    };

    scheduleDecision();

    return () => {
      cancelled = true;
      if (timerId !== null) {
        clearTimeout(timerId);
      }
    };
  }, [bounds, hidden, blurred, reducedMotion, held, dragging, hasReaction, hasMode]);

  const pose: CompanionPose = dragging
    ? 'surprised'
    : reaction
      ? REACTION_POSE[reaction.event]
      : blurred
        ? 'sleep'
        : basePose;

  return { x, facing, pose, setHeld, dragTo, setDragging };
}
