const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");
module.exports = async function testChatDelete(load, web) {
  const fromWeb = createRequire(path.join(web, "package.json"));
  const ts = fromWeb("typescript");
  const { chatApi } = load(path.join(web, "src/apis/chat.api.ts"));
  const { http } = load(path.join(web, "src/lib/http.ts"));
  const originalRequest = http.request;
  const requests = [];
  http.request = async (url, options) => { requests.push([url, options.method]); return new Response(null, { status: 204 }); };
  try { await chatApi.delete("chat/id"); assert.deepEqual(requests, [["/chats/chat%2Fid", "DELETE"]]); }
  finally { http.request = originalRequest; }

  let state, running = null, fail = false, pendingDelete;
  let deleted = [], cache, removed = [];
  const store = { getState: () => state, setState: patch => Object.assign(state, patch) };
  const client = {
    cancelQueries: async () => {},
    removeQueries: ({ queryKey }) => removed.push(queryKey),
    setQueryData: (_key, update) => { cache = update(cache); },
    invalidateQueries: async () => {},
  };
  const mocks = {
    react: { useRef: value => ({ current: value }) },
    "@tanstack/react-query": { useQueryClient: () => client, useQuery: () => ({ data: cache }) },
    "@/apis/chat.api": { chatApi: { delete: async id => { deleted.push(id); if (fail) throw Error("delete failed"); if (pendingDelete) await pendingDelete; } } },
    "./chat-store": { useChatStore: store },
    "./card-store": { useCardStore: { getState: () => ({ running }) } },
    "@/text/chat": { chatText: { deleteFailed: "delete failed" } },
  };
  const module = { exports: {} };
  const source = ts.transpileModule(fs.readFileSync(path.join(web, "src/components/features/chat/use-chat.ts"), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  vm.runInThisContext(`(function(require,module,exports){${source}\n})`)(id => mocks[id] || {}, module, module.exports);
  function fixture(active = "one") {
    cache = [{ id: "one" }, { id: "two" }]; deleted = []; removed = []; fail = false; pendingDelete = null; running = null;
    state = { id: active, streaming: false, error: "", messages: [], reset: () => { state.id = ""; state.messages = []; } };
    return module.exports.useChat();
  }
  let chat = fixture();
  assert.equal(await chat.remove("one"), true);
  assert.equal(state.id, ""); assert.deepEqual(cache, [{ id: "two" }]); assert.deepEqual(removed, [["chat", "one"]]);
  chat = fixture("two"); await chat.remove("one"); assert.equal(state.id, "two");
  chat = fixture(); fail = true;
  assert.equal(await chat.remove("one"), false); assert.equal(state.id, "one"); assert.equal(cache.length, 2); assert.equal(state.error, "delete failed");
  chat = fixture(); state.streaming = true;
  assert.equal(await chat.remove("one"), false); assert.deepEqual(deleted, []);
  state.streaming = false; running = "plan";
  assert.equal(await chat.remove("one"), false); assert.deepEqual(deleted, []);
  chat = fixture(); let release;
  pendingDelete = new Promise(resolve => { release = resolve; });
  const first = chat.remove("one");
  assert.equal(await chat.remove("one"), false);
  assert.equal(await chat.send("Có", true), false);
  release(); assert.equal(await first, true); assert.deepEqual(deleted, ["one"]);
  console.log("PASS chat deletion: DELETE encoding, active/other chat, cache cleanup, failure retention, busy and duplicate guards");
};
