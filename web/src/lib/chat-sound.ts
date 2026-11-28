const CHAT_SOUND_KEY = "ai-hub-chat-sound";
const CHAT_SOUND_VOLUME_KEY = "ai-hub-chat-sound-volume";
const DEFAULT_VOLUME = 60;
let audioContext: AudioContext | undefined;

export function chatSoundEnabled(): boolean {
  try { return typeof window === "undefined" || window.localStorage.getItem(CHAT_SOUND_KEY) !== "0"; }
  catch { return true; }
}

export function setChatSoundEnabled(on: boolean): void {
  try { window.localStorage.setItem(CHAT_SOUND_KEY, on ? "1" : "0"); }
  catch { /* Storage unavailable. */ }
}

function normalizeVolume(volume: number): number {
  return Number.isFinite(volume) ? Math.min(100, Math.max(0, Math.round(volume))) : DEFAULT_VOLUME;
}

export function chatSoundVolume(): number {
  try {
    const stored = typeof window === "undefined" ? null : window.localStorage.getItem(CHAT_SOUND_VOLUME_KEY);
    return stored?.trim() ? normalizeVolume(Number(stored)) : DEFAULT_VOLUME;
  } catch { return DEFAULT_VOLUME; }
}

export function setChatSoundVolume(volume: number): void {
  try { window.localStorage.setItem(CHAT_SOUND_VOLUME_KEY, String(normalizeVolume(volume))); }
  catch { /* Storage unavailable. */ }
}

export async function playChatSound(kind: "done" | "question"): Promise<void> {
  if (typeof window === "undefined" || !chatSoundEnabled()) return;
  const peak = 0.15 * chatSoundVolume() / 100;
  if (peak <= 0) return;
  try {
    const context = audioContext ??= new window.AudioContext();
    if (context.state === "suspended") await context.resume();

    const frequencies = kind === "done" ? [660, 880] : [880, 660, 880];
    const duration = kind === "done" ? 0.12 : 0.09;
    const interval = kind === "done" ? 0.11 : 0.1;
    const startTime = context.currentTime;
    for (const [index, frequency] of frequencies.entries()) {
      const start = startTime + index * interval;
      const end = start + duration;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = "sine";
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.linearRampToValueAtTime(peak, start + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, end);
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.onended = () => {
        try { oscillator.disconnect(); gain.disconnect(); }
        catch { /* Audio cleanup is best effort. */ }
      };
      oscillator.start(start);
      oscillator.stop(end);
    }
  } catch { /* Audio may be unavailable or blocked. */ }
}
