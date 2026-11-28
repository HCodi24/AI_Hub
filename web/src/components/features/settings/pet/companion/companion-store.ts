import { create } from 'zustand';
import { companionText } from '@/text/companion';

export type CompanionEvent =
  | 'remembered'
  | 'done'
  | 'question'
  | 'error'
  | 'greet'
  | 'sent'
  | 'stopped'
  | 'newChat'
  | 'switchChat'
  | 'deleted'
  | 'attached'
  | 'welcomeBack'
  | 'refocus'
  | 'dizzy'
  | 'themeChanged';

export type CompanionMode = 'typing' | 'thinking' | 'working';

export interface CompanionReaction {
  event: CompanionEvent;
  text: string;
  id: number;
}

const REACTION_DURATION_MS = 2500;
const TYPING_TIMEOUT_MS = 4000;

let nextReactionId = 0;
let clearTimer: ReturnType<typeof setTimeout> | null = null;
let typingTimer: ReturnType<typeof setTimeout> | null = null;

/** Zustand store holding the companion pet's transient reaction and persistent mode. */
export const useCompanion = create<{
  reaction: CompanionReaction | null;
  mode: CompanionMode | null;
}>(() => ({
  reaction: null,
  mode: null,
}));

/**
 * Trigger a companion reaction from anywhere (inside or outside React).
 * Auto-clears after ~2.5 s only if no newer reaction has replaced it.
 */
export function companionReact(event: CompanionEvent): void {
  const id = ++nextReactionId;
  useCompanion.setState({
    reaction: { event, text: companionText[event], id },
  });

  if (clearTimer !== null) {
    clearTimeout(clearTimer);
  }

  clearTimer = setTimeout(() => {
    clearTimer = null;
    useCompanion.setState((state) => (state.reaction?.id === id ? { reaction: null } : state));
  }, REACTION_DURATION_MS);
}

/**
 * Set or clear the companion's persistent mode.
 * `'typing'` auto-resets to `null` after 4000 ms without further `'typing'` updates;
 * any other value clears the typing timer immediately.
 */
export function setCompanionMode(mode: CompanionMode | null): void {
  if (typingTimer !== null) {
    clearTimeout(typingTimer);
    typingTimer = null;
  }

  useCompanion.setState({ mode });

  if (mode === 'typing') {
    typingTimer = setTimeout(() => {
      typingTimer = null;
      useCompanion.setState((state) => (state.mode === 'typing' ? { mode: null } : state));
    }, TYPING_TIMEOUT_MS);
  }
}
