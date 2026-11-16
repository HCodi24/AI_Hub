// Offline R2 behavior tests (debate card state, memory undo, SSE terminal event); no provider process or network access.
const assert = require("node:assert/strict");
const path = require("node:path");
module.exports = async function testR2(load, web) {
  const src = name => load(path.join(web, "src", name + ".ts"));
  const { DebateSchema } = src("schema/debates.schema");
  const { debateReducer } = src("components/features/debate/debate-reducer");
  const { memoryNotice } = src("components/features/memory/memory-events");
  const { http } = src("lib/http");
  const { postSse } = src("lib/sse");
  let live = { record: DebateSchema.parse({ id: "debate" }), activity: "", phase: "round", round: 1, live: true };
  for (const [event, data] of [["round", { round: 1 }], ["delta", { side: "codex", round: 1, text: "Xin chào" }], ["log", { side: "codex", round: 1, text: "Token: vào 123 ra 45" }], ["turn", { side: "claude", round: 1, turn: { text: "Đồng ý", agree: true, elapsed_s: 4 } }], ["judging", { model: "judge" }], ["verdict", { decision: "Kết hợp", plan_steps: ["Kiểm tra"] }]]) live = debateReducer(live, event, data);
  assert.equal(live.record.rounds[0].codex.text, "Xin chào");
  assert.equal(live.record.rounds[0].codex.tokens, 168);
  assert.equal(live.record.rounds[0].claude.agree, true);
  assert.equal(live.phase, "judge");
  live = debateReducer(live, "done", { ...live.record, status: "done", elapsed_s: 9 });
  assert.equal(live.live, false); assert.equal(live.record.elapsed_s, 9);
  assert.equal(DebateSchema.parse({ id: "old" }).elapsed_s, 0);
  const requests = [];
  const sse = events => new Response(events.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify(data)}\n\n`).join(""), { headers: { "content-type": "text/event-stream" } });
  const originalFetch = global.fetch;
  // Route every request through the mocked fetch below (earlier tests replaced the shared request method).
  http.request = (url, init) => fetch(`/api${url}`, init);
  global.fetch = async (url, init = {}) => {
    requests.push({ url, body: init.body ? JSON.parse(init.body) : {} });
    if (url.includes("/restore")) return Response.json({ ok: true });
    throw new Error(`Unexpected mock request ${url}`);
  };
  try {
    const notice = memoryNotice({ scope: "r2", status: "done", action: "archived" }, "C:/fixture");
    assert.match(notice.text, /Đã đóng scope r2/); await notice.action();
    assert.match(requests.at(-1).url, /r2\/restore\?folder=C%3A%2Ffixture/);
    assert.equal(memoryNotice({ scope: "r2", status: "active", action: "updated" }, "x").action, undefined);
    http.request = async () => sse([["done", { id: "d" }]]);
    await postSse("/debates", {}, new AbortController().signal, () => {}, "done");
    http.request = async () => sse([["log", { text: "partial" }]]);
    await assert.rejects(postSse("/debates", {}, new AbortController().signal, () => {}, "done"));
    console.log("PASS R2: debate reducer, memory undo, missing terminal event");
  } finally { global.fetch = originalFetch; }
};
