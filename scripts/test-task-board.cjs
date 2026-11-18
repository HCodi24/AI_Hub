// Offline contract test of the live task board of a MutiS High delegation: rendering and the chat store events.
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
const chatText = load(src("text/chat.ts")).chatText;
const schema = load(src("schema/chat.schema.ts"));
const { TaskBoardView } = load(src("components/features/chat/task-board.tsx"), {
  "@/text/chat": { chatText }, "./chat-store": { useChatStore: () => [] },
  "@/components/features/pets/pet-avatar": { PetAvatar: () => null }, "@/lib/pet-choice": { kindFromModel: model => model },
  // Only the Sol model has a chosen pet in this test; a row with a pet leaves the model name out.
  "@/components/features/pets/use-pet-id": { usePetId: (_, model) => model === "gpt-6.1-sol" ? "my-pet" : undefined },
  "@/components/features/composer/model-menu": { modelLabel: model => ({ "gpt-6-luna": "GPT-6 Tester", "gpt-6.1-sol": "GPT-6.1 Sol" })[model] || model },
});
const React = fromWeb("react");
const render = tasks => renderToStaticMarkup(React.createElement(TaskBoardView, { tasks }));

assert.equal(render([]), "", "no tasks, no board");
const html = render([
  { title: "A", state: "done", attempt: 1, check: "PASS" },
  { title: "B", state: "checking", attempt: 1, check: null },
  { title: "C", state: "fixing", attempt: 2, check: "FAIL" },
  { title: "D", state: "waiting", attempt: 0 },
  { title: "E", state: "failed", attempt: 4, check: "FAIL" },
  { title: "F", state: "takeover", attempt: 4, check: "FAIL" },
  { title: "G", state: "coding", attempt: 1, actor: "Codex", model: "gpt-6-luna" },
  { title: "H", state: "checking", attempt: 1, actor: "Tester", model: "gpt-6-luna", checker: "Tester", checker_model: "gpt-6-luna" },
  { title: "I", state: "coding", attempt: 1, actor: "DeepSeek", model: "deepseek-v4" },
  { title: "J", state: "checking", attempt: 1, actor: "Codex", model: "gpt-6-luna", checker: "Tester", checker_model: "gpt-6.1-sol", check: "PASS" },
]);
for (const wanted of ["Tiến độ các task", "Xong", "Đang kiểm tra", "Đang sửa", "lần 2", "Kiểm tra FAIL", "Kiểm tra PASS", "Chờ", "Lỗi", "Đã bàn giao", 'data-state="failed"',
  "Codex · GPT-6 Tester đang code", "GPT-6 Tester đang kiểm tra", "deepseek-v4 đang code"])
  assert.ok(html.includes(wanted), `missing ${wanted}`);
assert.ok(!html.includes("lần 1"), "the first attempt is not shown as a retry");
assert.ok(!html.includes("Codex CLI"), "the exact model is shown, not the generic source name");
assert.ok(!html.includes("GPT-6.1 Sol"), "a row with a pet leaves the model name out");
assert.ok(html.includes("Đang kiểm tra") && html.includes("Kiểm tra PASS"), "a row with a pet keeps the state and the verdict");
assert.equal((html.match(/\bspin\b/g) || []).length, 6, "only the six busy rows spin");

// The stream event keeps the board; a new message clears it. Unknown extra fields must not break parsing.
const parsed = schema.StreamDataSchema.parse({ tasks: [{ title: "A", state: "coding", attempt: 1, check: null, extra: 1 }] });
assert.deepEqual(parsed.tasks.map(t => t.state), ["coding"]);
const store = load(src("components/features/chat/chat-store.ts"), { "@/schema/chat.schema": schema }).useChatStore;
store.getState().event("tasks", parsed);
assert.equal(store.getState().tasks.length, 1);
store.getState().finish();
assert.equal(store.getState().tasks[0].state, "stopped", "a row still busy when the stream ends is shown as stopped");
store.getState().begin("next request");
assert.equal(store.getState().tasks.length, 0, "a new request starts with an empty board");
console.log("PASS task board: states, retries, Tester verdicts, busy rows and store events");
