// Offline contract test of the live Luna cards: the checker_activity events, the chat store and the rendered card.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");
const web = path.resolve(__dirname, "../web");
const fromWeb = createRequire(path.join(web, "package.json"));
const ts = fromWeb("typescript");
const { renderToStaticMarkup } = fromWeb("react-dom/server");
function load(file, mocks = {}) {
  const module = { exports: {} };
  const source = ts.transpileModule(fs.readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const requireLocal = id => id in mocks ? mocks[id] : id.endsWith(".module.css") ? { default: new Proxy({}, { get: (_, key) => key }) } : fromWeb(id);
  vm.runInThisContext(`(function(require,module,exports){${source}\n})`, { filename: file })(requireLocal, module, module.exports);
  return module.exports;
}
const src = file => path.join(web, "src", file);
const schema = load(src("schema/chat.schema.ts"));
const storeModule = load(src("components/features/chat/chat-store.ts"), { "@/schema/chat.schema": schema });
const store = storeModule.useChatStore;
const chatText = load(src("text/chat.ts"));
const { LunaActivityView } = load(src("components/features/chat/luna-activity.tsx"), {
  "./chat-store": { useChatStore: () => [] },
  "@/text/chat": chatText,
  // The pet needs a react-query client; the card contract only needs to know which pet state it was given.
  "@/components/features/pets/pet-avatar": { PetAvatar: ({ provider, state }) => fromWeb("react").createElement("i", { "data-pet": provider, "data-pet-state": state }) },
  "./chat-activity": load(src("components/features/chat/chat-activity.tsx"), { "@/text/chat": chatText }),
});
const React = fromWeb("react");
const render = checkers => renderToStaticMarkup(React.createElement(LunaActivityView, { checkers }));
const activity = (id, phase, extra = {}) => schema.StreamDataSchema.parse({
  checker_activity: { id, phase, title: "Task A", attempt: 1, model: "gpt-5-codex", provider_name: "Luna", text: "", ...extra },
});
const send = (id, phase, extra) => store.getState().event("checker_activity", activity(id, phase, extra));
const cards = () => store.getState().checkers;

// 1. A start event alone produces a visible, running Luna card, before any verdict text exists.
assert.equal(render([]), "", "no checks, no card");
store.getState().begin("build it");
send("task a#1", "start");
assert.equal(cards().length, 1, "start opens one card");
assert.equal(cards()[0].state, "running");
assert.equal(cards()[0].provider_name, "Luna");
assert.equal(cards()[0].model, "gpt-5-codex");
const started = render(cards());
for (const wanted of ["Luna đang kiểm tra", "Luna", "gpt-5-codex", "Task A", "lần 1", "Đang xử lý", 'data-pet="codex_cli"', 'data-pet-state="working"', 'data-checker-id="task a#1"'])
  assert.ok(started.includes(wanted), `missing ${wanted}`);
assert.ok(started.includes("Activity (0)"), "the collapsed Activity list is the same one a chat message uses");
assert.equal(store.getState().messages.at(-1).content, "", "the card never writes into the chat message");

// 2. Progress merges into its own card only, and the retained log stays bounded.
send("task a#1", "progress", { text: "chạy cargo test" });
assert.equal(cards()[0].progress.at(-1), "chạy cargo test");
assert.ok(render(cards()).includes("chạy cargo test"), "the latest progress is visible");
for (let index = 0; index < 10; index += 1) send("task a#1", "progress", { text: `bước ${index}` });
assert.equal(cards().length, 1, "progress never opens a second card");
assert.equal(cards()[0].progress.length, 6, "retained progress is bounded");
assert.equal(cards()[0].progress.at(-1), "bước 9");

// 3. Interleaved worker events touch neither the card nor the worker message.
store.getState().event("turn", schema.StreamDataSchema.parse({ provider_kind: "deepseek", provider_name: "DeepSeek", model: "deepseek-chat" }));
store.getState().event("delta", schema.StreamDataSchema.parse({ text: "worker text" }));
send("task a#1", "progress", { text: "đọc src/lib.rs" });
store.getState().event("log", schema.StreamDataSchema.parse({ text: "worker log" }));
const worker = store.getState().messages.at(-1);
assert.equal(worker.content, "worker text", "checker events never append to the worker message");
assert.equal(worker.provider, "deepseek", "the worker keeps its own identity");
assert.equal(worker.model, "deepseek-chat");
assert.deepEqual(worker.logs, ["worker log"], "checker progress never lands in the worker logs");
assert.equal(cards()[0].progress.at(-1), "đọc src/lib.rs", "worker events never touch the card");
assert.equal(cards()[0].model, "gpt-5-codex", "the card keeps Luna's model");

// 4. Two checks and a retry stay independent.
send("task b#1", "start", { title: "Task B" });
send("task a#2", "start", { title: "Task A", attempt: 2 });
assert.deepEqual(cards().map(card => card.id), ["task a#1", "task b#1", "task a#2"]);
send("task b#1", "progress", { title: "Task B", text: "chạy npm test" });
assert.equal(cards()[0].progress.at(-1), "đọc src/lib.rs", "another check's progress stays out");
assert.equal(cards()[1].progress.at(-1), "chạy npm test");
assert.equal(cards()[2].progress.length, 0, "the retry starts empty");
assert.ok(render([cards()[2]]).includes("lần 2"), "the retry shows its attempt");

// 5. An error marks only its card; completion removes only its card.
send("task b#1", "error", { title: "Task B", text: "hết thời gian" });
assert.equal(cards()[1].state, "failed");
assert.equal(cards()[0].state, "running", "a failure elsewhere leaves the running card alone");
assert.ok(render(cards()).includes("hết thời gian"));
send("task a#1", "done");
assert.deepEqual(cards().map(card => card.id), ["task b#1", "task a#2"], "done removes exactly its own card");
assert.ok(!render(cards()).includes('data-checker-id="task a#1"'));

// 6. Lifecycle: a stream end stops a still running card, a new request and a reset clear them.
store.getState().finish();
assert.equal(cards().find(card => card.id === "task a#2").state, "stopped", "a card still running at the end is stopped");
assert.equal(cards().find(card => card.id === "task b#1").state, "failed", "a failed card keeps its state");
assert.ok(render(cards()).includes("Đã dừng"));
store.getState().begin("next request");
assert.equal(cards().length, 0, "a new request starts with no cards");
send("task c#1", "start", { title: "Task C" });
store.getState().reset();
assert.equal(cards().length, 0, "a reset clears the cards");
send("task d#1", "start", { title: "Task D" });
store.getState().load({ id: "11111111-1111-4111-8111-111111111111", title: "saved", updated_at: 0, dev_chat: false, folder: "",
  provider_id: "codex_cli", model: "gpt-5-codex", permission: "write", effort: "medium", file_tools: "none", messages: [] });
assert.equal(cards().length, 0, "loading another chat never shows a stale card");

// 7. The event is optional and unknown fields do not break parsing.
assert.equal(schema.StreamDataSchema.parse({}).checker_activity, undefined);
const parsed = schema.StreamDataSchema.parse({ checker_activity: { id: "x#1", phase: "start", extra: 1 } });
assert.equal(parsed.checker_activity.provider_name, "Luna", "the provider label defaults to Luna");
assert.equal(parsed.checker_activity.attempt, 0);


// The transport envelope must match the Rust producer, not just frontend fixtures.
const rust = fs.readFileSync(path.resolve(__dirname, "../src/providers/delegate.rs"), "utf8");
const helper = rust.slice(rust.indexOf("fn checker_activity("), rust.indexOf("fn checker_progress("));
assert.match(helper, /data:\s*serde_json::json!\(\{\s*"checker_activity":\s*\{/);
const backendBody = { checker_activity: { id: "wire#1", phase: "start", title: "Wire task", attempt: 1, model: "luna", provider_name: "Luna", text: "" } };
store.getState().event("checker_activity", schema.StreamDataSchema.parse(backendBody));
assert.equal(cards().at(-1).id, "wire#1");
backendBody.checker_activity.phase = "progress";
backendBody.checker_activity.text = "Read file";
store.getState().event("checker_activity", schema.StreamDataSchema.parse(backendBody));
assert.equal(cards().at(-1).progress.at(-1), "Read file");
backendBody.checker_activity.phase = "done";
store.getState().event("checker_activity", schema.StreamDataSchema.parse(backendBody));
assert.equal(cards().length, 0);

console.log("PASS luna activity: wire envelope, live visibility, isolation and cleanup");
