import { useCallback, useEffect, useRef, useState } from "react";

type Journal<T> = { revision: number; status: "pending" | "confirmed"; value: T };
type Options<T> = {
  key: string;
  initial: T;
  validate: (value: unknown) => value is T;
  equal?: (left: T, right: T) => boolean;
  save: (value: T) => Promise<unknown>;
  debounceMs?: number;
  onError: (error: unknown) => void;
  onSaved?: () => void;
};

const same = <T,>(left: T, right: T) => JSON.stringify(left) === JSON.stringify(right);
const resourceQueues = new Map<string, Promise<void>>();
const resourceRevisions = new Map<string, number>();
const resourceConfirmedRevisions = new Map<string, number>();
const resourceSnapshots = new Map<string, { revision: number; value: unknown; pending: boolean }>();

function nextResourceRevision(key: string) {
  const revision = (resourceRevisions.get(key) ?? 0) + 1;
  resourceRevisions.set(key, revision);
  return revision;
}

function adoptResourceRevision(key: string, revision: number) {
  resourceRevisions.set(key, Math.max(resourceRevisions.get(key) ?? 0, revision));
}

function markResourceRevisionConfirmed(key: string, revision: number) {
  resourceConfirmedRevisions.set(key, Math.max(resourceConfirmedRevisions.get(key) ?? 0, revision));
}

function isLatestResourceRevision(key: string, revision: number) {
  return resourceRevisions.get(key) === revision;
}

/** Serializes writes and related operations across every hook instance for one resource. */
export async function withSettingsResourceQueue<T>(key: string, action: () => Promise<T>): Promise<T> {
  const previous = resourceQueues.get(key) ?? Promise.resolve();
  const current = previous.then(action);
  const tail = current.then(() => undefined, () => undefined);
  resourceQueues.set(key, tail);
  try { return await current; }
  finally { if (resourceQueues.get(key) === tail) resourceQueues.delete(key); }
}

/** Keeps the latest edit durable and serializes writes for one settings resource. */
export function useSettingsAutosave<T>({ key, initial, validate, equal = same, save, debounceMs = 400, onError, onSaved }: Options<T>) {
  const sharedInitial = resourceSnapshots.get(key);
  const restored = sharedInitial?.pending ? sharedInitial.value as T : initial;
  const [draft, setDraft] = useState(restored);
  const [busy, setBusy] = useState(false);
  const latest = useRef(restored);
  const revision = useRef(sharedInitial?.revision ?? 0);
  const dirty = useRef(Boolean(sharedInitial?.pending));
  const eligible = useRef(Boolean(sharedInitial?.pending));
  const hydrated = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const running = useRef(false);
  const inFlightRevision = useRef(-1);
  const queued = useRef(false);
  const failedRevision = useRef<number | null>(null);
  const mounted = useRef(true);
  const saveRef = useRef(save); saveRef.current = save;
  const errorRef = useRef(onError); errorRef.current = onError;
  const savedRef = useRef(onSaved); savedRef.current = onSaved;
  const opts = useRef({ key, validate, equal, debounceMs }); opts.current = { key, validate, equal, debounceMs };

  const writeJournal = useCallback((value: T, rev: number) => {
    try { if (typeof window !== "undefined") window.localStorage.setItem(opts.current.key, JSON.stringify({ revision: rev, status: "pending", value } satisfies Journal<T>)); }
    catch { /* Storage can be disabled or full; editing and API saves still work. */ }
  }, []);
  const clearJournal = useCallback((rev: number) => {
    try {
      if (typeof window === "undefined") return;
      const raw = window.localStorage.getItem(opts.current.key);
      if (!raw) return;
      const parsed: unknown = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && "revision" in parsed && parsed.revision === rev) {
        try { window.localStorage.setItem(opts.current.key, JSON.stringify({ ...parsed, status: "confirmed" })); }
        catch { /* Try removal even when writing the confirmed marker fails. */ }
        try { window.localStorage.removeItem(opts.current.key); }
        catch { /* A confirmed marker, when writable, prevents replay after reload. */ }
      }
    } catch { /* A malformed or inaccessible journal must not break settings. */ }
  }, []);
  const removeJournal = useCallback(() => {
    try { if (typeof window !== "undefined") window.localStorage.removeItem(opts.current.key); }
    catch { /* Storage failures never prevent in-memory edits or API saves. */ }
  }, []);

  const runQueue = useCallback(async () => {
    if (running.current) { if (revision.current !== inFlightRevision.current) queued.current = true; return; }
    running.current = true;
    if (mounted.current) setBusy(true);
    try {
      do {
        queued.current = false;
        const sentValue = latest.current;
        const sentRevision = revision.current;
        inFlightRevision.current = sentRevision;
        try {
          let skipped = false;
          const confirmed = await withSettingsResourceQueue(opts.current.key, async () => {
            if (!isLatestResourceRevision(opts.current.key, sentRevision) || (resourceConfirmedRevisions.get(opts.current.key) ?? 0) >= sentRevision) {
              skipped = true;
              return undefined;
            }
            const result = await saveRef.current(sentValue);
            resourceConfirmedRevisions.set(opts.current.key, sentRevision);
            const snapshot = resourceSnapshots.get(opts.current.key);
            if (snapshot?.revision === sentRevision) resourceSnapshots.set(opts.current.key, { revision: sentRevision, value: result ?? sentValue, pending: false });
            return result;
          });
          if (skipped) continue;
          if (failedRevision.current === sentRevision) failedRevision.current = null;
          clearJournal(sentRevision);
          if (revision.current === sentRevision) {
            latest.current = (confirmed ?? sentValue) as T;
            if (mounted.current) setDraft(latest.current);
            dirty.current = false;
          }
          savedRef.current?.();
        } catch (error) {
          failedRevision.current = sentRevision;
          errorRef.current(error);
          if (revision.current !== sentRevision && eligible.current) queued.current = true;
        }
        if (revision.current !== sentRevision && eligible.current) queued.current = true;
      } while (queued.current);
    } finally {
      running.current = false;
      if (mounted.current) setBusy(false);
    }
  }, [clearJournal]);

  const schedule = useCallback((value: T, immediate = false, persist = true) => {
    latest.current = value;
    setDraft(value);
    if (!persist) {
      revision.current = nextResourceRevision(opts.current.key);
      resourceSnapshots.set(opts.current.key, { revision: revision.current, value, pending: false });
      eligible.current = false;
      dirty.current = false;
      queued.current = false;
      if (timer.current) { clearTimeout(timer.current); timer.current = null; }
      removeJournal();
      return;
    }
    if (!opts.current.validate(value)) {
      revision.current = nextResourceRevision(opts.current.key);
      resourceSnapshots.set(opts.current.key, { revision: revision.current, value, pending: false });
      eligible.current = false;
      dirty.current = false;
      queued.current = false;
      if (timer.current) { clearTimeout(timer.current); timer.current = null; }
      removeJournal();
      return;
    }
    eligible.current = true;
    revision.current = nextResourceRevision(opts.current.key);
    resourceSnapshots.set(opts.current.key, { revision: revision.current, value, pending: true });
    dirty.current = true;
    writeJournal(value, revision.current);
    if (timer.current) clearTimeout(timer.current);
    if (immediate) void runQueue();
    else timer.current = setTimeout(() => { timer.current = null; void runQueue(); }, opts.current.debounceMs);
  }, [removeJournal, runQueue, writeJournal]);

  const update = useCallback((change: (current: T) => T, immediate = false, persist = true) => {
    schedule(change(latest.current), immediate, persist);
  }, [schedule]);

  const flush = useCallback(async () => {
    if (timer.current) { clearTimeout(timer.current); timer.current = null; }
    if (!dirty.current) return true;
    const targetRevision = revision.current;
    await runQueue();
    while (running.current) await new Promise(resolve => setTimeout(resolve, 0));
    // A skipped write is successful only if this edit (or its replacement) was confirmed.
    const confirmed = resourceConfirmedRevisions.get(opts.current.key) ?? 0;
    return confirmed >= targetRevision && confirmed >= revision.current;
  }, [runQueue]);

  const exclusive = useCallback(async <R,>(task: () => Promise<R>) => {
    const saved = await flush();
    if (!saved) return undefined;
    while (running.current) await new Promise(resolve => setTimeout(resolve, 0));
    running.current = true;
    if (mounted.current) setBusy(true);
    try { return await withSettingsResourceQueue(opts.current.key, task); }
    finally {
      running.current = false;
      if (mounted.current) setBusy(false);
      if (queued.current) void runQueue();
    }
  }, [flush, runQueue]);

  const hydrate = useCallback((serverValue: T) => {
    if (!opts.current.validate(serverValue)) return;
    if (!hydrated.current) {
      hydrated.current = true;
      try {
        const raw = typeof window === "undefined" ? null : window.localStorage.getItem(opts.current.key);
        const parsed: unknown = raw ? JSON.parse(raw) : null;
        if (parsed && typeof parsed === "object" && "revision" in parsed && typeof parsed.revision === "number" && Number.isInteger(parsed.revision) && parsed.revision > 0 && "value" in parsed && opts.current.validate(parsed.value) &&
          (!("status" in parsed) || parsed.status === "pending" || parsed.status === "confirmed")) {
          const shared = resourceSnapshots.get(opts.current.key);
          if (shared && shared.revision > parsed.revision && opts.current.validate(shared.value)) {
            adoptResourceRevision(opts.current.key, shared.revision);
            revision.current = shared.revision;
            latest.current = shared.value;
            setDraft(shared.value);
            dirty.current = shared.pending;
            eligible.current = shared.pending;
            if (shared.pending) {
              writeJournal(shared.value, shared.revision);
              timer.current = setTimeout(() => { timer.current = null; void runQueue(); }, 0);
            } else clearJournal(parsed.revision);
            return;
          }
          if ("status" in parsed && parsed.status === "confirmed") {
            adoptResourceRevision(opts.current.key, parsed.revision);
            markResourceRevisionConfirmed(opts.current.key, parsed.revision);
            if (!shared || shared.revision <= parsed.revision) resourceSnapshots.set(opts.current.key, { revision: parsed.revision, value: serverValue, pending: false });
            window.localStorage.removeItem(opts.current.key);
            dirty.current = false; eligible.current = false;
            if (timer.current) { clearTimeout(timer.current); timer.current = null; }
            latest.current = serverValue; setDraft(serverValue);
            return;
          }
          const journal = { ...parsed, status: "pending" } as Journal<T>;
          if (journal.revision <= (resourceConfirmedRevisions.get(opts.current.key) ?? 0) || journal.revision < (resourceRevisions.get(opts.current.key) ?? 0)) {
            clearJournal(journal.revision);
            dirty.current = false; eligible.current = false;
            if (timer.current) { clearTimeout(timer.current); timer.current = null; }
            latest.current = serverValue; setDraft(serverValue);
            return;
          }
          adoptResourceRevision(opts.current.key, journal.revision);
          revision.current = journal.revision;
          if (opts.current.equal(journal.value, serverValue)) {
            markResourceRevisionConfirmed(opts.current.key, journal.revision);
            resourceSnapshots.set(opts.current.key, { revision: journal.revision, value: serverValue, pending: false });
            clearJournal(journal.revision);
            dirty.current = false; eligible.current = false;
            if (timer.current) { clearTimeout(timer.current); timer.current = null; }
            latest.current = serverValue; setDraft(serverValue);
          } else {
            const shared = resourceSnapshots.get(opts.current.key);
            const restoredValue = shared?.pending && shared.revision >= journal.revision ? shared.value as T : journal.value;
            latest.current = restoredValue; setDraft(restoredValue); dirty.current = true; eligible.current = true;
            resourceSnapshots.set(opts.current.key, { revision: revision.current, value: restoredValue, pending: true });
            writeJournal(restoredValue, revision.current);
            timer.current = setTimeout(() => { timer.current = null; void runQueue(); }, 0);
          }
          return;
        }
        if (raw) window.localStorage.removeItem(opts.current.key);
      } catch { /* Invalid JSON or unavailable storage falls back to server state. */ }
    }
    if (!dirty.current) { latest.current = serverValue; setDraft(serverValue); }
  }, [clearJournal, runQueue, writeJournal]);

  useEffect(() => {
    mounted.current = true;
    const snapshot = resourceSnapshots.get(opts.current.key);
    if (snapshot?.pending && snapshot.revision === revision.current && (resourceConfirmedRevisions.get(opts.current.key) ?? 0) < snapshot.revision) {
      timer.current = setTimeout(() => { timer.current = null; void runQueue(); }, 0);
    }
    return () => { mounted.current = false; };
  }, [runQueue]);
  return { draft, busy, schedule, update, flush, exclusive, hydrate, dirty };
}
