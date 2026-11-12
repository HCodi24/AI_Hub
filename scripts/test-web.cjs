// Offline contract tests. Load TypeScript through the already installed compiler.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");
const web = path.resolve(__dirname, "../web");
const fromWeb = createRequire(path.join(web, "package.json"));
const ts = fromWeb("typescript");
const cache = new Map();
function load(file) {
  file = path.resolve(file);
  if (cache.has(file)) return cache.get(file);
  const module = { exports: {} };
  cache.set(file, module.exports);
  const source = ts.transpileModule(fs.readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const localRequire = (id) => id.startsWith("@/")
    ? load(path.join(web, "src", id.slice(2) + ".ts"))
    : id.startsWith(".") ? load(path.resolve(path.dirname(file), id + ".ts")) : fromWeb(id);
  vm.runInThisContext(`(function(require,module,exports){${source}\n})`, { filename: file })(localRequire, module, module.exports);
  return module.exports;
}
const text = load(path.join(web, "src/text/chat.ts")).chatText;
require("./test-chat-choices.cjs")(load, web);
require("./test-chat-questions.cjs")(load, web);
require("./test-chat-pet-state.cjs")(load, web);
assert.equal(text.newChat, "Chat mới");
const schema = load(path.join(web, "src/schema/chat.schema.ts"));
const { useChatStore } = load(path.join(web, "src/components/features/chat/chat-store.ts"));
const state = () => useChatStore.getState();
state().initialize("C:/fixture", "fixture", "mock");
state().begin("Xin chào");
for (const [event, data] of [
  ["delta", { text: "Chào bạn" }], ["reasoning", { text: "Suy luận" }],
  ["log", { text: "Token: 12" }], ["diff", { text: "+ mới\n- cũ" }],
  ["status", { provider_name: "Mock", model: "mock" }],
  ["heartbeat", { elapsed_s: 3, idle_s: 1 }],
  ["memory", { scope: "r1", status: "active", action: "updated" }],
  ["error", { text: "Lỗi giả lập" }], ["done", {}],
]) state().event(event, schema.StreamDataSchema.parse(data));
const message = state().messages.at(-1);
assert.equal(message.content, "Chào bạn");
assert.equal(message.reasoning, "Suy luận");
assert.equal(message.logs.length, 2);
assert.equal(message.error, "Lỗi giả lập");
assert.equal(state().status.elapsed_s, 3);
assert.equal(state().status.provider_name, "Mock");
state().finish(); assert.equal(state().streaming, false);
const record = schema.ChatRecordSchema.parse({ id: state().id, title: "Mock", provider_id: "p", model: "m", folder: "C:/saved", messages: [], updated_at: 1 });
state().load(record); assert.equal(state().settings.folder, "C:/saved");
assert.equal(record.permission, "write");
const { hideMemory } = load(path.join(web, "src/lib/markdown.ts"));
assert.equal(hideMemory("Hi<memory>{private}</memory>!"), "Hi!");
assert.equal(hideMemory("Hi<memory>unfinished"), "Hi");
assert.equal(hideMemory("<memory>x</memory>A<memory>y</memory>"), "A");
const http = load(path.join(web, "src/lib/http.ts")).http;
const { postSse } = load(path.join(web, "src/lib/sse.ts"));
async function run() {
  const encoder = new TextEncoder();
  const wire = encoder.encode('event: delta\r\ndata: {"text":"Tiếng Việt"}\r\n\r\nevent: done\ndata: {}\n\n');
  http.request = async () => new Response(new ReadableStream({ start(controller) {
    // Split every byte, including UTF-8 characters and CRLF frame delimiters.
    for (const byte of wire) controller.enqueue(new Uint8Array([byte]));
    controller.close();
  } }), { headers: { "content-type": "text/event-stream" } });
  const events = [];
  await postSse("/chat", {}, new AbortController().signal, (event, data) => events.push([event, data]));
  assert.deepEqual(events, [["delta", { text: "Tiếng Việt" }], ["done", {}]]);
  http.request = async () => new Response('event: delta\ndata: {"text":"partial"}\n\n', { headers: { "content-type": "text/event-stream" } });
  await assert.rejects(postSse("/chat", {}, new AbortController().signal, () => {}), { message: text.streamError });
  http.request = async () => new Response('data: {}', { headers: { "content-type": "application/json" } });
  await assert.rejects(postSse("/chat", {}, new AbortController().signal, () => {}), { message: text.streamError });
  await require("./test-web-r2.cjs")(load, web);
  await require("./test-chat-delete.cjs")(load, web);
  await require("./test-import-file.cjs")(load, web);
  console.log("PASS web contracts: UTF-8/CRLF SSE, missing done, content-type, store events, history defaults, hidden memory");
}
run().catch((error) => { console.error(error); process.exitCode = 1; });
