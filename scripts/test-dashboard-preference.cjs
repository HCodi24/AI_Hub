/**
 * Assertions for the persisted Dashboard toggle (use-dashboard-preference.ts).
 * Uses the installed TypeScript transpiler plus a mocked React/window/storage harness.
 * Run: node scripts/test-dashboard-preference.cjs
 */
const fs = require("fs");
const path = require("path");
const { createRequire } = require("module");

// TypeScript is installed in web/node_modules, so resolve it from the web package.
const webDir = path.join(__dirname, "..", "web");
const fromWeb = createRequire(path.join(webDir, "package.json"));
const ts = fromWeb("typescript");

const HOOK_PATH = path.join(webDir, "src", "components", "features", "chat", "use-dashboard-preference.ts");
const KEY = "ai-hub-dashboard-enabled";

const source = fs.readFileSync(HOOK_PATH, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2019, esModuleInterop: true },
  fileName: "use-dashboard-preference.ts",
}).outputText;

let passed = 0;
const failures = [];
function check(name, condition, detail) {
  if (condition) { passed += 1; return; }
  failures.push(detail ? `${name} (${detail})` : name);
}
function equal(name, actual, expected) {
  check(name, Object.is(actual, expected), `expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

function createStorage(initial) {
  const map = new Map(Object.entries(initial || {}));
  const stats = { writes: 0 };
  const storage = {
    stats,
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => { stats.writes += 1; map.set(k, String(v)); },
    removeItem: (k) => { map.delete(k); },
    clear: () => { map.clear(); },
    key: (i) => Array.from(map.keys())[i] ?? null,
    get length() { return map.size; },
    raw: map,
  };
  return storage;
}

function createWindow(storage) {
  const listeners = new Set();
  return {
    localStorage: storage,
    addEventListener: (type, fn) => { if (type === "storage") listeners.add(fn); },
    removeEventListener: (type, fn) => { if (type === "storage") listeners.delete(fn); },
    dispatchStorage(key, newValue) {
      const event = { key, newValue, storageArea: storage };
      for (const fn of Array.from(listeners)) fn(event);
    },
    listenerCount: () => listeners.size,
  };
}

function loadHook(reactMock) {
  const mod = { exports: {} };
  const requireMock = (id) => {
    if (id === "react") return reactMock;
    throw new Error(`unexpected require: ${id}`);
  };
  new Function("exports", "require", "module", "__filename", "__dirname", compiled)(
    mod.exports, requireMock, mod, "use-dashboard-preference.js", __dirname,
  );
  return mod.exports.useDashboardPreference;
}

/** Minimal hook harness: state slots, deferred effects, cleanup tracking. */
function createHarness(win) {
  const previousWindow = global.window;
  global.window = win;
  const states = [];
  const cleanups = [];
  let cursor = 0;
  let pending = [];
  const reactMock = {
    useState(initial) {
      const index = cursor++;
      if (!(index in states)) states[index] = typeof initial === "function" ? initial() : initial;
      const set = (value) => { states[index] = typeof value === "function" ? value(states[index]) : value; };
      return [states[index], set];
    },
    useEffect(fn) { pending.push(fn); },
    useCallback(fn) { return fn; },
  };
  const useDashboardPreference = loadHook(reactMock);

  function render() {
    cursor = 0;
    pending = [];
    const [enabled, setEnabled] = useDashboardPreference();
    return { enabled, setEnabled };
  }
  function runEffects() {
    const list = pending;
    pending = [];
    for (const fn of list) {
      const cleanup = fn();
      if (typeof cleanup === "function") cleanups.push(cleanup);
    }
  }
  return {
    mount() { const r = render(); runEffects(); return r; },
    rerender() { const r = render(); runEffects(); return r; },
    unmount() { while (cleanups.length) cleanups.pop()(); },
    get enabled() { return states[0]; },
    restore() { global.window = previousWindow; },
  };
}

function scenario(initialStorage) {
  const storage = createStorage(initialStorage);
  const win = createWindow(storage);
  const harness = createHarness(win);
  return { storage, win, harness };
}

// 1. Saved true is restored on mount and on a fresh page/remount.
{
  const { storage, harness } = scenario({ [KEY]: "true" });
  harness.mount();
  equal("saved true restored on mount", harness.enabled, true);
  harness.unmount();
  harness.restore();
  const second = scenario({ [KEY]: "true" });
  second.harness.mount();
  equal("saved true restored on new page", second.harness.enabled, true);
  equal("restoring true does not rewrite storage", second.storage.stats.writes, 0);
  second.harness.unmount();
  second.harness.restore();
  check("saved true kept in storage", storage.getItem(KEY) === "true");
}

// 2. Saved false is restored.
{
  const { harness } = scenario({ [KEY]: "false" });
  harness.mount();
  equal("saved false restored", harness.enabled, false);
  harness.unmount();
  harness.restore();
}

// 3. Toggling persists both values immediately.
{
  const { storage, harness } = scenario({});
  const { setEnabled } = harness.mount();
  equal("default false without saved value", harness.enabled, false);
  equal("no write on mount", storage.stats.writes, 0);
  setEnabled(true);
  equal("toggle on updates state", harness.enabled, true);
  equal("toggle on persists true", storage.getItem(KEY), "true");
  setEnabled(false);
  equal("toggle off updates state", harness.enabled, false);
  equal("toggle off persists false", storage.getItem(KEY), "false");
  equal("two writes for two toggles", storage.stats.writes, 2);
  harness.unmount();
  harness.restore();
}

// 4. Mount effects (including StrictMode double effects) never clobber a saved true.
{
  const { storage, harness } = scenario({ [KEY]: "true" });
  harness.mount();
  harness.rerender();
  harness.rerender();
  equal("repeated mount effects keep true", harness.enabled, true);
  equal("repeated mount effects do not write", storage.stats.writes, 0);
  equal("storage still true", storage.getItem(KEY), "true");
  harness.unmount();
  harness.restore();
}

// 5. Cross-tab storage events update state without writing back.
{
  const { storage, win, harness } = scenario({});
  harness.mount();
  win.dispatchStorage(KEY, "true");
  equal("cross-tab true applied", harness.enabled, true);
  win.dispatchStorage(KEY, "false");
  equal("cross-tab false applied", harness.enabled, false);
  win.dispatchStorage(KEY, "true");
  win.dispatchStorage(KEY, null); // key removed in another tab
  equal("key removal resets to false", harness.enabled, false);
  win.dispatchStorage(KEY, "true");
  win.dispatchStorage(null, null); // localStorage.clear()
  equal("clear resets to false", harness.enabled, false);
  win.dispatchStorage("other-key", "true");
  equal("unrelated key ignored", harness.enabled, false);
  equal("storage events never write back", storage.stats.writes, 0);
  harness.unmount();
  harness.restore();
}

// 6. Listener cleanup on unmount.
{
  const { win, harness } = scenario({});
  harness.mount();
  equal("listener registered", win.listenerCount(), 1);
  harness.unmount();
  equal("listener removed", win.listenerCount(), 0);
  harness.restore();
}

// 7. Malformed saved values fall back to false without crashing.
{
  for (const raw of ["yes", "1", "", "TRUE", "{}", "null"]) {
    const { harness } = scenario({ [KEY]: raw });
    harness.mount();
    equal(`malformed value ${JSON.stringify(raw)} -> false`, harness.enabled, false);
    harness.unmount();
    harness.restore();
  }
}

// 8. Blocked storage: no crash, toggles still work in memory.
{
  const storage = createStorage({});
  storage.getItem = () => { throw new Error("blocked"); };
  storage.setItem = () => { throw new Error("blocked"); };
  const win = createWindow(storage);
  const harness = createHarness(win);
  let threw = false;
  try {
    const { setEnabled } = harness.mount();
    equal("blocked storage defaults to false", harness.enabled, false);
    setEnabled(true);
    equal("blocked storage still toggles on", harness.enabled, true);
    setEnabled(false);
    equal("blocked storage still toggles off", harness.enabled, false);
  } catch (error) {
    threw = true;
    failures.push(`blocked storage threw: ${error && error.message}`);
  }
  check("blocked storage does not throw", !threw);
  harness.unmount();
  harness.restore();
}

if (failures.length) {
  console.error(`dashboard preference: ${failures.length} failed, ${passed} passed`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exitCode = 1;
} else {
  console.log(`dashboard preference: ${passed} assertions passed`);
}
