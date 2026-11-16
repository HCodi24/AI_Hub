// Offline event/measurement tests for the scroll-follow hook; no browser or provider required.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const ts = require("../web/node_modules/typescript");
const frames = new Map();
const timers = new Map();
let sequence = 0;
let refs = [], cursor = 0;
const effects = [];
const observers = [];
class Host {
  constructor() {
    this.listeners = new Map();
    this.scrollHeight = 1000; this.clientHeight = 200; this.scrollTop = 800; this.clientWidth = 300;
    this.ownerDocument = new Events();
  }
  addEventListener(name, fn) { this.listeners.set(name, fn); }
  removeEventListener(name) { this.listeners.delete(name); }
  emit(name, data = {}) { this.listeners.get(name)?.({ target: this, ...data }); }
  contains(node) { return node === this; }
  closest() { return null; }
  matches() { return false; }
  querySelectorAll() { return []; }
  getBoundingClientRect() { return { left: 0, top: 0, width: 316 }; }
}
class Events {
  constructor() { this.listeners = new Map(); }
  addEventListener(name, fn) { this.listeners.set(name, fn); }
  removeEventListener(name) { this.listeners.delete(name); }
  emit(name, data) { this.listeners.get(name)?.(data); }
}
class Observer {
  constructor(callback) { this.callback = callback; this.nodes = new Set(); observers.push(this); }
  observe(node) { this.nodes.add(node); }
  unobserve(node) { this.nodes.delete(node); }
  disconnect() { this.nodes.clear(); }
}
const moduleCopy = { exports: {} };
const filename = path.resolve(__dirname, "../web/src/components/features/chat/use-chat-scroll-follow.ts");
const code = ts.transpileModule(fs.readFileSync(filename, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
vm.runInNewContext(code, {
  module: moduleCopy, exports: moduleCopy.exports,
  require: () => ({
    useRef: value => refs[cursor++] ||= { current: value },
    useCallback: fn => fn,
    useLayoutEffect: fn => effects.push(fn),
  }),
  HTMLElement: Host, Element: Host, ResizeObserver: Observer, MutationObserver: Observer,
  requestAnimationFrame: fn => { const id = ++sequence; frames.set(id, fn); return id; },
  cancelAnimationFrame: id => frames.delete(id),
  window: { setTimeout: fn => { const id = ++sequence; timers.set(id, fn); return id; }, clearTimeout: id => timers.delete(id) },
});
function render(id, count) {
  cursor = 0;
  const hook = moduleCopy.exports.useChatScrollFollow(id, count);
  effects.splice(0).forEach(fn => fn());
  return hook;
}
function flush() {
  const batch = [...frames.values()]; frames.clear(); batch.forEach(fn => fn());
}
const host = new Host();
let hook = render("first", 2);
const calls = [];
hook.virtuosoRef.current = { scrollTo: options => { calls.push(options.top); host.scrollTop = options.top; } };
hook.scrollerRef(host);
const resize = observers[0];
flush(); assert.equal(calls.length, 0);
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 900, "same-message growth follows the measured bottom");
host.scrollHeight += 100; resize.callback();
host.emit("wheel", { deltaY: -80 });
host.scrollTop = 800; host.emit("scroll"); flush();
assert.equal(host.scrollTop, 800, "upward input cancels a pending follow frame");
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 800, "growth preserves the reader position");
host.emit("touchstart", { touches: [{ clientY: 200 }] });
host.emit("touchmove", { touches: [{ clientY: 100 }] });
host.scrollTop = 950; host.emit("scroll");
host.emit("touchend");
host.scrollTop = 1100; host.emit("scroll"); flush();
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 1200, "inertial return to bottom restores following");
host.emit("pointerdown", { pointerId: 7, clientX: 310, clientY: 60 });
host.scrollTop = 500; host.emit("scroll");
host.scrollTop = 1200; host.emit("scroll");
host.ownerDocument.emit("pointerup", { pointerId: 7 }); flush();
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 1300, "scrollbar return restores following");
const footer = new Host(); footer.matches = selector => selector.includes("data-chat-virtuoso-footer");
observers[1].callback([{ addedNodes: [footer], removedNodes: [], target: host }]);
assert.ok(resize.nodes.has(footer), "new footer is observed");
host.scrollHeight += 50; resize.callback(); flush();
assert.equal(host.scrollTop, 1350, "footer growth is included in the scroll extent");
host.clientHeight -= 50; resize.callback(); flush();
assert.equal(host.scrollTop, 1400, "viewport shrink follows while pinned");
host.emit("keydown", { key: "PageUp" });
host.scrollTop = 700; host.emit("scroll");
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 700, "keyboard history navigation disables following");
host.emit("keydown", { key: "End" });
host.scrollTop = 1500; host.emit("scroll"); flush();
const button = new Host(); button.closest = () => button;
host.contains = node => node === host || node === button;
host.emit("keydown", { target: button, key: " " });
host.scrollHeight += 100; resize.callback(); flush();
assert.equal(host.scrollTop, 1600, "interactive controls do not change reader intent");
host.scrollHeight += 100; resize.callback();
hook = render("second", 2); flush();
assert.equal(host.scrollTop, 1600, "old callback cannot scroll a new chat");
hook.scrollerRef(null);
assert.equal(host.listeners.size, 0);
assert.equal(host.ownerDocument.listeners.size, 0);
assert.equal(frames.size, 0);
assert.equal(timers.size, 0);
const short = new Host(); short.scrollHeight = 100; short.scrollTop = 0;
hook.virtuosoRef.current = { scrollTo: options => { short.scrollTop = options.top; } };
hook.scrollerRef(short);
short.emit("wheel", { deltaY: -50 }); flush();
short.scrollHeight = 400; observers.at(-2).callback(); flush();
assert.equal(short.scrollTop, 200, "non-scrollable content stays pinned when it grows");
hook.scrollerRef(null);
console.log("PASS: chat scroll-follow event and measurement scenarios");
