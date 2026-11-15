const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");
const web = path.resolve(__dirname, "../web");
const fromWeb = createRequire(path.join(web, "package.json"));
const ts = fromWeb("typescript");
function load(relative, mocks = {}) {
  const source = ts.transpileModule(fs.readFileSync(path.join(web, "src", relative), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const mod = { exports: {} };
  new Function("require", "module", "exports", source)(id => {
    if (id in mocks) return mocks[id];
    throw new Error(`Unexpected import ${id}`);
  }, mod, mod.exports);
  return mod.exports;
}
const api = load("lib/chat-token-timeline.ts");
const usage = (input, model = "m") => ({ reports: 1, input, output: 2, total: input + 2,
  models: [{ model, input, output: 2, total: input + 2, reports: 1 }] });
let samples = api.appendTokenSample([], usage(10), 1000);
assert.equal(samples[0].at, 1000);
assert.equal(samples[0].models[0].total, 12);
assert.equal(api.appendTokenSample(samples, usage(10), 2000), samples);
samples = api.appendTokenSample(samples, usage(20), 999);
assert.equal(samples[1].at, 1001);
assert.equal(samples[1].models[0].total, 22);
assert.equal(api.appendTokenSample(samples, { ...usage(0), reports: 0 }, 3000), samples);
assert.equal(api.appendTokenSample([], usage(0, ""), 0)[0].models[0].model, "");
assert.deepEqual(api.parseTokenHistory(JSON.stringify(samples)), samples);
for (const raw of ["bad", "{}", '[{"at":1,"models":[]}]', JSON.stringify([samples[0], samples[0]]),
  JSON.stringify([{ at: 1, models: [{ model: "m", input: -1, output: 2, total: 1 }] }])]) {
  assert.deepEqual(api.parseTokenHistory(raw), []);
}
let bounded = [];
for (let i = 0; i < 510; i++) bounded = api.appendTokenSample(bounded, usage(i), i);
assert.equal(bounded.length, 500);
assert.equal(bounded[0].at, 10);
// Legacy browser-storage helpers stay defensive; the Dashboard hook no longer uses them.
const storage = new Map();
global.window = { localStorage: { getItem: k => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) } };
api.writeTokenHistory("a", samples);
api.writeTokenHistory("b", bounded);
api.writeTokenHistory("", samples);
assert.equal(storage.size, 2);
assert.deepEqual(api.readTokenHistory("a"), samples);
assert.equal(api.readTokenHistory("b").length, 500);
global.window.localStorage.getItem = () => { throw Error("blocked"); };
global.window.localStorage.setItem = () => { throw Error("blocked"); };
assert.deepEqual(api.readTokenHistory("a"), []);
assert.doesNotThrow(() => api.writeTokenHistory("a", samples));
delete global.window;
assert.deepEqual(api.readTokenHistory("a"), []);

// --- Hook: backend history only, exercised with a mocked API and mocked timers.
// Top-level await is invalid in a .cjs file, so every async scenario lives in main().
async function main() {
  const realSetTimeout = global.setTimeout;
  const realClearTimeout = global.clearTimeout;
  const realWindow = global.window;
  try {
    let timers = [];
    global.setTimeout = (fn, ms) => { const timer = { fn, ms, cleared: false }; timers.push(timer); return timer; };
    global.clearTimeout = timer => { if (timer) timer.cleared = true; };
    const flush = () => new Promise(resolve => setImmediate(resolve));
    const pending = () => timers.filter(timer => !timer.cleared);
    // One-shot timers: a fired timer is removed before it runs, so it can never fire twice.
    const fireTimers = async () => {
      const due = pending();
      timers = timers.filter(timer => !timer.cleared && !due.includes(timer));
      for (const timer of due) timer.fn();
      await flush();
    };
    const deferred = () => { let resolve, reject; const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; }); return { promise, resolve, reject }; };
    const multi = rows => ({ reports: rows.length,
      input: rows.reduce((sum, row) => sum + row.input, 0), output: rows.reduce((sum, row) => sum + row.output, 0),
      total: rows.reduce((sum, row) => sum + row.input + row.output, 0),
      models: rows.map(row => ({ model: row.model, input: row.input, output: row.output, total: row.input + row.output, reports: 1 })) });

    let current = null;
    const react = {
      useRef(initial) { const owner = current; const slot = owner.slot++; if (!(slot in owner.refs)) owner.refs[slot] = { current: initial }; return owner.refs[slot]; },
      useState(initial) {
        const owner = current;
        const slot = owner.slot++;
        if (!(slot in owner.states)) owner.states[slot] = initial;
        // The setter belongs to the instance that created it: async updates must never read a global.
        const set = value => { owner.states[slot] = typeof value === "function" ? value(owner.states[slot]) : value; };
        return [owner.states[slot], set];
      },
      useEffect(fn, deps) { const owner = current; const slot = owner.slot++; owner.effects[slot] = { fn, deps }; },
    };
    const calls = [];
    let handler = async () => [];
    const chatApiMock = { chatApi: { tokenHistory: (id, signal) => { calls.push({ id, signal }); return handler(id, signal); } } };
    const { useTokenTimeline, TOKEN_TIMELINE_POLL_MS } = load("components/features/chat/use-token-timeline.ts", {
      react, "@/apis/chat.api": chatApiMock,
    });
    function mountHook() {
      const instance = { slot: 0, refs: {}, states: {}, effects: {}, deps: {}, cleanups: {} };
      const render = (chatId, value) => {
        const previous = current;
        current = instance;
        instance.slot = 0;
        let result;
        try {
          result = useTokenTimeline(chatId, value);
        } finally {
          current = previous;
        }
        for (const key of Object.keys(instance.effects)) {
          const effect = instance.effects[key];
          const deps = instance.deps[key];
          if (deps && deps.length === effect.deps.length && deps.every((item, index) => Object.is(item, effect.deps[index]))) continue;
          if (instance.cleanups[key]) instance.cleanups[key]();
          instance.deps[key] = effect.deps;
          instance.cleanups[key] = effect.fn();
        }
        return result;
      };
      const unmount = () => {
        for (const key of Object.keys(instance.cleanups)) { instance.cleanups[key]?.(); delete instance.cleanups[key]; }
      };
      return { render, unmount };
    }

    // Initial loading: one request, no samples, no invented timestamps.
    const first = deferred();
    handler = () => first.promise;
    const hook = mountHook();
    let view = hook.render("chat-a", usage(10));
    assert.deepEqual(view, { samples: [], status: "loading" });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].id, "chat-a");
    assert.ok(calls[0].signal instanceof AbortSignal, "the request is abortable");
    assert.equal(TOKEN_TIMELINE_POLL_MS, 3000, "modest polling interval");
    await fireTimers();
    assert.equal(calls.length, 1, "no overlapping request while one is in flight");

    // Success: backend samples become the chart source.
    first.resolve(samples);
    await flush();
    view = hook.render("chat-a", usage(10));
    assert.equal(view.status, "ready");
    assert.deepEqual(view.samples, samples);
    assert.equal(pending().length, 1);
    assert.equal(pending()[0].ms, 3000);

    // A text-only delta (same counts, new object) must not restart the request.
    const before = calls.length;
    hook.render("chat-a", usage(10));
    assert.equal(calls.length, before, "stable usage identity keeps the poll alive");
    // Changed reported counts do refetch.
    const second = deferred();
    handler = () => second.promise;
    hook.render("chat-a", usage(20));
    assert.equal(calls.length, before + 1);
    second.resolve(samples);
    await flush();
    assert.equal(hook.render("chat-a", usage(20)).status, "ready");

    // Per-model identity: a redistribution between models refetches, a text-only delta does not.
    const beforeModels = calls.length;
    hook.render("chat-a", multi([{ model: "m", input: 20, output: 2 }]));
    assert.equal(calls.length, beforeModels, "the same per-model counts keep the poll alive");
    const redistributed = deferred();
    handler = () => redistributed.promise;
    hook.render("chat-a", multi([{ model: "m", input: 10, output: 2 }, { model: "n", input: 10, output: 0 }]));
    assert.equal(calls.length, beforeModels + 1, "a per-model redistribution refetches");
    redistributed.resolve(samples);
    await flush();
    assert.equal(hook.render("chat-a", multi([{ model: "m", input: 10, output: 2 }, { model: "n", input: 10, output: 0 }])).status, "ready");
    const back = deferred();
    handler = () => back.promise;
    hook.render("chat-a", usage(20));
    assert.equal(calls.length, beforeModels + 2, "returning to the previous per-model identity refetches");
    back.resolve(samples);
    await flush();
    assert.equal(hook.render("chat-a", usage(20)).status, "ready");

    // Successful empty history is ready, never loading forever.
    handler = async () => [];
    await fireTimers();
    view = hook.render("chat-a", usage(20));
    assert.deepEqual(view, { samples: [], status: "ready" });

    // Failure: error status, and the next poll retries automatically.
    handler = async () => { throw new Error("offline"); };
    await fireTimers();
    view = hook.render("chat-a", usage(20));
    assert.deepEqual(view, { samples: [], status: "error" });
    assert.equal(pending().length, 1, "a retry is scheduled after a failure");
    handler = async () => samples;
    await fireTimers();
    view = hook.render("chat-a", usage(20));
    assert.deepEqual(view, { samples, status: "ready" });

    // Transient error keeps the last successful samples of the current chat.
    handler = async () => { throw new Error("offline"); };
    await fireTimers();
    view = hook.render("chat-a", usage(20));
    assert.equal(view.status, "error");
    assert.deepEqual(view.samples, samples, "last successful samples survive a transient error");

    // Chat switch: no old-chat samples, stale response ignored, old request aborted.
    // A real pending chat-a request first: a usage-key change starts it while chat-a is current.
    const stale = deferred();
    handler = () => stale.promise;
    const staleIndex = calls.length;
    hook.render("chat-a", usage(30));
    assert.equal(calls.length, staleIndex + 1, "the usage-key change starts a fresh chat-a request");
    assert.equal(calls.at(-1).id, "chat-a");
    const staleSignal = calls.at(-1).signal;
    assert.equal(staleSignal.aborted, false, "the chat-a request is still pending");
    // Switching chats aborts that pending request and starts an independent one for chat-b.
    const own = deferred();
    handler = () => own.promise;
    view = hook.render("chat-b", usage(20));
    assert.deepEqual(view, { samples: [], status: "loading" }, "another chat's samples are never exposed");
    assert.equal(calls.length, staleIndex + 2);
    assert.equal(calls.at(-1).id, "chat-b");
    assert.ok(staleSignal.aborted, "the previous chat's request is aborted");
    assert.equal(calls.at(-1).signal.aborted, false, "the new chat's request is independent");
    // The aborted chat-a response must not leak samples or status into chat-b.
    stale.resolve(samples);
    await flush();
    assert.deepEqual(hook.render("chat-b", usage(20)), { samples: [], status: "loading" }, "stale response ignored");
    // chat-b's own response is resolved independently and becomes its history.
    own.resolve([{ at: 5000, models: [{ model: "b", input: 1, output: 1, total: 2 }] }]);
    await flush();
    view = hook.render("chat-b", usage(20));
    assert.equal(view.status, "ready");
    assert.deepEqual(view.samples.map(sample => sample.at), [5000]);

    // Cleanup: unmount aborts the in-flight request and stops polling.
    const last = deferred();
    handler = () => last.promise;
    hook.render("chat-b", usage(30));
    const inFlight = calls.at(-1);
    hook.unmount();
    assert.ok(inFlight.signal.aborted, "unmount aborts the request");
    assert.equal(pending().length, 0, "unmount clears the poll timer");
    const afterUnmount = calls.length;
    await fireTimers();
    assert.equal(calls.length, afterUnmount, "no request after unmount");

    // Empty chat id: no request for an invalid URL, no polling, ready and empty.
    const emptyHook = mountHook();
    const beforeEmpty = calls.length;
    assert.deepEqual(emptyHook.render("", usage(10)), { samples: [], status: "ready" });
    await flush();
    assert.equal(calls.length, beforeEmpty, "an empty chat id never requests /chats//token-history");
    assert.equal(pending().length, 0, "an empty chat id never schedules a poll");
    await fireTimers();
    assert.equal(calls.length, beforeEmpty, "no request after firing timers with an empty chat id");
    assert.deepEqual(emptyHook.render("", usage(10)), { samples: [], status: "ready" });
    emptyHook.unmount();

    // No browser storage is touched by the hook.
    let storageAccess = 0;
    global.window = { localStorage: { getItem: () => { storageAccess++; return null; }, setItem: () => { storageAccess++; } } };
    const clean = mountHook();
    handler = async () => samples;
    clean.render("chat-c", usage(10));
    await flush();
    assert.equal(clean.render("chat-c", usage(10)).status, "ready");
    await fireTimers();
    clean.unmount();
    assert.equal(storageAccess, 0, "the hook never reads or writes localStorage");

    // Source guards: no client observation timestamps, no browser history import.
    const hookSource = fs.readFileSync(path.join(web, "src/components/features/chat/use-token-timeline.ts"), "utf8");
    for (const banned of ["Date.now", "localStorage", "readTokenHistory", "writeTokenHistory", "appendTokenSample"])
      assert.ok(!hookSource.includes(banned), `hook must not use ${banned}`);
    assert.match(hookSource, /chatApi\.tokenHistory\(/);
    assert.match(hookSource, /AbortController/);
    assert.match(hookSource, /if \(!chatId\)/, "an empty chat id short-circuits before requesting");
  } finally {
    // Restore every global mock even when an assertion fails.
    global.setTimeout = realSetTimeout;
    global.clearTimeout = realClearTimeout;
    if (realWindow === undefined) delete global.window; else global.window = realWindow;
  }
}

main().then(() => {
  console.log("PASS token timeline: snapshots, dedupe, timestamps, validation, bounds, storage helpers and backend-history hook (loading, success, empty, failure/retry, per-model identity, empty chat id, cleanup, chat isolation, no storage)");
}, error => {
  console.error(error && error.stack ? error.stack : error);
  process.exitCode = 1;
});
