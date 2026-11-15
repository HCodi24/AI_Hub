// Offline contract test of the Dashboard token chart: pure aggregation, schema metadata and store stamping.
// Loads the TypeScript sources through the already installed compiler (no new dependencies).
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");
const web = path.resolve(__dirname, "../web");
const fromWeb = createRequire(path.join(web, "package.json"));
const ts = fromWeb("typescript");
const cache = new Map();

function load(file, mocks = {}) {
  file = path.resolve(file);
  if (!Object.keys(mocks).length && cache.has(file)) return cache.get(file);
  const module = { exports: {} };
  if (!Object.keys(mocks).length) cache.set(file, module.exports);
  const source = ts.transpileModule(fs.readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const localRequire = id => {
    if (id in mocks) return mocks[id];
    if (id.endsWith(".module.css")) return { default: new Proxy({}, { get: (_, key) => key }) };
    if (id.startsWith("@/")) return load(path.join(web, "src", id.slice(2) + ".ts"));
    if (id.startsWith(".")) return load(path.resolve(path.dirname(file), id + ".ts"));
    return fromWeb(id);
  };
  vm.runInThisContext(`(function(require,module,exports){${source}\n})`, { filename: file })(localRequire, module, module.exports);
  return module.exports;
}

const src = file => path.join(web, "src", file);
const schema = load(src("schema/chat.schema.ts"));
const { chatTokenUsage, lastTokenReport, formatTokens } = load(src("lib/chat-token-usage.ts"));
const assistant = (model, logs) => schema.ChatMessageSchema.parse({ role: "assistant", content: "x", model, logs });
const user = (logs) => schema.ChatMessageSchema.parse({ role: "user", content: "x", logs });

// --- Anchored parsing of the three existing log shapes, cache/reasoning never added again.
assert.deepEqual(lastTokenReport(["Token: vào 1200 (cache 300), ra 45 (suy luận 10)"]), { input: 1200, output: 45 });
assert.deepEqual(lastTokenReport(["Token: vào 7, ra 3, chi phí $0.01"]), { input: 7, output: 3 });
assert.deepEqual(lastTokenReport(["Token: vào 12, ra 3"]), { input: 12, output: 3 });
assert.deepEqual(lastTokenReport(["  Token: vào 5, ra 6  "]), { input: 5, output: 6 });

// --- Repeated snapshots of one message: only the last valid report counts.
assert.deepEqual(lastTokenReport(["Token: vào 100, ra 10", "Token: vào 1200 (cache 300), ra 45 (suy luận 10)"]), { input: 1200, output: 45 });
assert.deepEqual(lastTokenReport(["Token: vào 100, ra 10", "Token: vào x, ra 3"]), { input: 100, output: 10 }, "a malformed tail falls back to the last valid report");

// --- Malformed, nonfinite, negative and unrelated logs are ignored.
for (const bad of [
  "Token: vào abc, ra 3",
  "Token: vào -5, ra 3",
  "Token: vào 12, ra -3",
  "Token: vào 1e999, ra 3",
  "Token: vào 12, ra 3 extra",
  "Token: 12",
  "Token: vào 12",
  "vào 12, ra 3",
  "Token: vào 12, ra 3, chi phí $0.01, ghi chú",
  "Token: vào 12.5, ra 3",
]) assert.equal(lastTokenReport([bad]), undefined, `must ignore: ${bad}`);

// --- Explicit zero reports are preserved, not treated as missing.
assert.deepEqual(lastTokenReport(["Token: vào 0, ra 0"]), { input: 0, output: 0 });
const zero = chatTokenUsage([assistant("m-zero", ["Token: vào 0, ra 0"])]);
assert.equal(zero.reports, 1);
assert.deepEqual(zero.models.map(g => [g.model, g.input, g.output, g.total]), [["m-zero", 0, 0, 0]]);

// --- Multiple models, exact grouping, shared totals.
const messages = [
  user(["Token: vào 999, ra 999"]),
  assistant("gpt-5", ["Token: vào 100, ra 10", "Token: vào 1200 (cache 300), ra 45 (suy luận 10)"]),
  assistant("claude-sonnet-4-5", ["Token: vào 7, ra 3, chi phí $0.01"]),
  assistant("gpt-5", ["Token: vào 12, ra 3"]),
  assistant("", ["Token: vào 40, ra 2"]),
  assistant("", ["Token: vào 5, ra 1"]),
  assistant("no-report", []),
  assistant("bad-report", ["Token: vào x, ra y"]),
  user(["Token: vào 500, ra 500"]),
];
const usage = chatTokenUsage(messages);
assert.equal(usage.reports, 5, "only assistant messages with a valid report count");
assert.equal(usage.input, 1200 + 7 + 12 + 40 + 5);
assert.equal(usage.output, 45 + 3 + 3 + 2 + 1);
assert.equal(usage.total, usage.input + usage.output);
assert.deepEqual(usage.models.map(g => g.model), ["gpt-5", "", "claude-sonnet-4-5"]);
const byModel = Object.fromEntries(usage.models.map(g => [g.model, g]));
assert.deepEqual([byModel["gpt-5"].input, byModel["gpt-5"].output, byModel["gpt-5"].reports], [1212, 48, 2]);
assert.deepEqual([byModel[""].input, byModel[""].output, byModel[""].reports], [45, 3, 2], "missing model stays a separate unknown group");
assert.deepEqual([byModel["claude-sonnet-4-5"].input, byModel["claude-sonnet-4-5"].output], [7, 3]);
assert.ok(!usage.models.some(g => g.model === "no-report" || g.model === "bad-report"), "no fabricated rows");

// --- Old missing-model reports never inherit the currently selected model.
const legacy = chatTokenUsage([assistant("", ["Token: vào 30, ra 4"])]);
assert.deepEqual(legacy.models.map(g => g.model), [""]);
assert.equal(legacy.models[0].input, 30);

// --- Empty chat has no reports and no rows.
assert.deepEqual(chatTokenUsage([]), { models: [], input: 0, output: 0, total: 0, reports: 0 });
assert.deepEqual(chatTokenUsage([user(["Token: vào 1, ra 1"])]).models, []);

// --- Exact, locale-stable formatting.
assert.equal(formatTokens(0), "0");
assert.equal(formatTokens(1200), "1.200");
assert.equal(formatTokens(1234567), "1.234.567");

// --- Backend metadata survives schema parsing (default empty, explicit value kept).
assert.equal(schema.ChatMessageSchema.parse({ role: "assistant", content: "" }).model, "");
assert.equal(schema.ChatMessageSchema.parse({ role: "assistant", content: "", model: "gpt-5" }).model, "gpt-5");
const record = schema.ChatRecordSchema.parse({ id: "8e4e00a4-60ef-4a76-b49c-86d2828ec998", title: "Tokens", provider_id: "p", model: "selected",
  folder: "", updated_at: 1, messages: [{ role: "assistant", content: "x", model: "gpt-5", logs: ["Token: vào 10, ra 2"] }, { role: "assistant", content: "y" }] });
assert.deepEqual(record.messages.map(m => m.model), ["gpt-5", ""]);
assert.equal(chatTokenUsage(record.messages).models[0].model, "gpt-5");

// --- API schema: valid rows, invalid rows, duplicate models, timestamp order/range, malformed bodies.
const httpModule = load(src("lib/http.ts"));
let requested = [];
let body = [];
const api = load(src("apis/chat.api.ts"), {
  "@/lib/http": { ...httpModule, http: { get: async (url) => { requested.push(url); return body; } } },
  "@/lib/sse": { postSse: async () => undefined },
  "@/schema/chat.schema": schema,
});
const { TokenHistorySchema } = api;
const row = (model, input, output) => ({ model, input, output, total: input + output });
const sampleAt = (at, models) => ({ at, models });
const valid = [sampleAt(1000, [row("b", 1, 2), row("a", 3, 4)]), sampleAt(2000, [row("a", 5, 6)])];
const parsedHistory = TokenHistorySchema.parse(valid);
assert.deepEqual(parsedHistory.map(s => s.at), [1000, 2000]);
assert.deepEqual(parsedHistory[0].models.map(r => r.model), ["a", "b"], "rows are sorted by model identity");
assert.deepEqual(parsedHistory[0].models[0], { model: "a", input: 3, output: 4, total: 7 });
assert.deepEqual(TokenHistorySchema.parse([]), [], "an empty history is valid");
assert.deepEqual(TokenHistorySchema.parse([sampleAt(0, [row("", 0, 0)])])[0].models[0],
  { model: "", input: 0, output: 0, total: 0 }, "zero counts and the unknown model are valid");
for (const [name, value] of [
  ["negative input", [sampleAt(1, [{ model: "m", input: -1, output: 2, total: 1 }])]],
  ["fractional output", [sampleAt(1, [{ model: "m", input: 1, output: 1.5, total: 2.5 }])]],
  ["total mismatch", [sampleAt(1, [{ model: "m", input: 1, output: 2, total: 4 }])]],
  ["missing total", [sampleAt(1, [{ model: "m", input: 1, output: 2 }])]],
  ["non-string model", [sampleAt(1, [{ model: 7, input: 1, output: 2, total: 3 }])]],
  ["no rows", [sampleAt(1, [])]],
  ["duplicate models", [sampleAt(1, [row("m", 1, 2), row("m", 3, 4)])]],
  ["duplicate unknown models", [sampleAt(1, [row("", 1, 2), row("", 3, 4)])]],
  ["repeated timestamp", [sampleAt(2000, [row("m", 1, 2)]), sampleAt(2000, [row("m", 1, 2)])]],
  ["descending timestamps", [sampleAt(2000, [row("m", 1, 2)]), sampleAt(1000, [row("m", 1, 2)])]],
  ["negative timestamp", [sampleAt(-1, [row("m", 1, 2)])]],
  ["fractional timestamp", [sampleAt(1.5, [row("m", 1, 2)])]],
  ["timestamp beyond the JS Date range", [sampleAt(8640000000000001, [row("m", 1, 2)])]],
  ["null sample", [null]],
  ["missing models", [{ at: 1 }]],
]) assert.equal(TokenHistorySchema.safeParse(value).success, false, `must reject ${name}`);
assert.equal(TokenHistorySchema.safeParse([sampleAt(8640000000000000, [row("m", 1, 2)])]).success, true,
  "the largest representable JS Date timestamp is accepted");
for (const malformed of [null, undefined, {}, "[]", 42, [{}], [{ at: "1", models: [row("m", 1, 2)] }]])
  assert.equal(TokenHistorySchema.safeParse(malformed).success, false, `must reject malformed body ${String(JSON.stringify(malformed))}`);

async function apiRequestScenarios() {
  requested = [];
  body = valid;
  const result = await api.chatApi.tokenHistory("chat id/1");
  assert.deepEqual(requested, ["/chats/chat%20id%2F1/token-history"], "the chat id is encoded in the history URL");
  assert.deepEqual(result.map(s => s.at), [1000, 2000]);
  for (const malformed of [null, {}, [{ at: 1, models: [] }], [{ at: 8640000000000001, models: [row("m", 1, 2)] }]]) {
    body = malformed;
    await assert.rejects(() => api.chatApi.tokenHistory("c"), error => error instanceof httpModule.ApiError,
      `a malformed body must throw instead of becoming []: ${String(JSON.stringify(malformed))}`);
  }
  body = [];
  assert.deepEqual(await api.chatApi.tokenHistory("c"), [], "an empty history is not an error");
}

// --- Backend snapshots rendered by the card: two points of the same three models.
const baseline = { at: 1700000000000, models: usage.models.map(({model,input,output,total}) => ({model,input,output,total})) };
const next = { at: baseline.at + 5000, models: baseline.models.map(r => ({...r, output: r.output + 1, total: r.total + 1})) };

// --- Rendered card: Vietnamese labels, exact numbers, textual equivalents and the honest empty state.
const React = fromWeb("react");
const { renderToStaticMarkup } = fromWeb("react-dom/server");
const chatText = load(src("text/chat.ts")).chatText;
const { TokenDashboardView } = load(src("components/features/chat/token-dashboard.tsx"), {
  "@/text/chat": { chatText }, "./chat-store": { useChatStore: () => [] }, "./use-token-timeline": { useTokenTimeline: () => [] },
});
const render = (list, samples = [], status = "ready") => renderToStaticMarkup(React.createElement(TokenDashboardView, { messages: list, samples, status }));
const emptyHtml = render([]);
assert.ok(emptyHtml.includes("Token phiên chat"), "title is always shown");
assert.ok(emptyHtml.includes("Chưa có dữ liệu token."), "honest empty state");
assert.ok(!emptyHtml.includes("Tổng đã ghi"), "no fabricated totals without reports");
assert.ok(emptyHtml.includes(chatText.tokenDashboard.historyEmpty), "ready + empty history is reported as empty");
assert.ok(!emptyHtml.includes(chatText.tokenDashboard.waiting), "a ready empty history is not loading");
assert.ok(!emptyHtml.includes('<svg'), "no chart without samples");
// Loading, ready-empty and error stay distinct even when the messages carry no reports.
const emptyLoading = render([], [], "loading");
assert.ok(emptyLoading.includes(chatText.tokenDashboard.waiting), "loading is shown without reports");
assert.ok(!emptyLoading.includes(chatText.tokenDashboard.historyEmpty), "loading is not reported as empty");
assert.ok(!emptyLoading.includes(chatText.tokenDashboard.historyError), "loading is not reported as an error");
assert.ok(!emptyLoading.includes('<svg'), "no chart while loading");
const emptyError = render([], [], "error");
assert.ok(emptyError.includes(chatText.tokenDashboard.historyError), "a failure is shown without reports");
assert.ok(!emptyError.includes(chatText.tokenDashboard.waiting), "a failure is not reported as loading");
assert.ok(!emptyError.includes(chatText.tokenDashboard.historyEmpty), "a failure is not reported as empty");
assert.ok(!emptyError.includes('<svg'), "no chart without samples");
assert.equal((emptyError.match(new RegExp(chatText.tokenDashboard.historyError, "g")) || []).length, 1, "the error note is not duplicated");
assert.equal((emptyHtml.match(new RegExp(chatText.tokenDashboard.historyEmpty, "g")) || []).length, 1, "the empty note is not duplicated");
// Backend samples stay visible even when the current message logs have no reports.
const serverOnly = render([], [baseline, next]);
assert.ok(serverOnly.includes(chatText.tokenDashboard.empty), "the local empty state is still honest");
assert.ok(!serverOnly.includes("Tổng đã ghi"), "no fabricated totals from server samples");
assert.match(serverOnly, /<svg/);
assert.equal((serverOnly.match(/<circle/g) || []).length, 6, "server samples are plotted without local reports");
assert.match(serverOnly, /<table/);
assert.ok(serverOnly.includes("gpt-5"), "the legend lists the server models");
assert.ok(!/NaN|Infinity/.test(serverOnly));
const serverOnlyError = render([], [baseline, next], "error");
assert.match(serverOnlyError, /<svg/, "a failure keeps the retained chart");
assert.equal((serverOnlyError.match(/<circle/g) || []).length, 6, "retained points survive a failure");
assert.ok(serverOnlyError.includes(chatText.tokenDashboard.historyError), "the failure is still announced");
assert.equal((serverOnlyError.match(new RegExp(chatText.tokenDashboard.historyError, "g")) || []).length, 1, "one error note only");
const serverOnlyLoading = render([], [baseline], "loading");
assert.match(serverOnlyLoading, /<svg/, "samples stay visible while a refresh is in flight");
assert.ok(!serverOnlyLoading.includes(chatText.tokenDashboard.waiting), "no loading note when samples exist");
const cardHtml = render(messages, [baseline, next]);
for (const wanted of [chatText.tokenDashboard.title, chatText.tokenDashboard.timeAxis,
  chatText.tokenDashboard.total("1.264", "54"), ">Vào<", ">Ra<", "gpt-5", "Không rõ model",
  "gpt-5: vào 1.212, ra 48", "Không rõ model: vào 45, ra 3", chatText.tokenDashboard.note])
  assert.ok(cardHtml.includes(wanted), 'missing ' + wanted);
assert.ok(!cardHtml.includes("no-report"));
assert.match(cardHtml, /<svg/);
assert.match(cardHtml, /d="M[^"]*H[^"]*V/);
assert.equal((cardHtml.match(/<circle/g) || []).length, 6);
assert.ok(!/NaN|Infinity/.test(cardHtml));
assert.match(cardHtml, /<table/);
const single = render(messages, [baseline]);
assert.equal((single.match(/<circle/g) || []).length, 3);
assert.ok(!/NaN|Infinity/.test(single));
const zeroHtml = render([assistant("m-zero", ["Token: vào 0, ra 0"])], [{at: baseline.at, models: [{model:"m-zero",input:0,output:0,total:0}]}]);
assert.match(zeroHtml, /<circle/);
assert.ok(!/NaN|Infinity/.test(zeroHtml));
const waiting = render(messages, [], "loading");
assert.ok(waiting.includes(chatText.tokenDashboard.waiting));
assert.ok(!waiting.includes('<svg'));
const late = render(messages, [baseline, {at: next.at, models: [...next.models, {model: "late", input:1,output:1,total:2}]}]);
assert.equal((late.match(/<circle/g) || []).length, 7, 'late model has no invented earlier point');
const collision = render(messages, [{at:baseline.at, models:[{model:"",input:0,output:0,total:0},{model:"unknown",input:1,output:0,total:1}]}]);
assert.equal((collision.match(/<circle/g) || []).length, 2);

// --- Store: status stamps the streaming reply, heartbeat never relabels, turn carries the event model.
const { useChatStore } = load(src("components/features/chat/chat-store.ts"), { "@/schema/chat.schema": schema });
const state = () => useChatStore.getState();
state().initialize("C:/fixture", "fixture", "selected-model");
state().begin("Xin chào");
state().event("status", schema.StreamDataSchema.parse({ provider_kind: "deepseek", model: "gpt-5" }));
assert.equal(state().messages.at(-1).model, "gpt-5", "status stamps the active reply");
assert.equal(state().messages.at(-1).provider, "deepseek");
state().event("heartbeat", schema.StreamDataSchema.parse({ model: "other-model", elapsed_s: 2 }));
assert.equal(state().messages.at(-1).model, "gpt-5", "heartbeat must not relabel the reply");
assert.equal(state().status.elapsed_s, 2);
state().event("delta", schema.StreamDataSchema.parse({ text: "Chào" }));
state().event("turn", schema.StreamDataSchema.parse({ provider_kind: "deepseek", model: "claude-sonnet-4-5" }));
const turn = state().messages.at(-1);
assert.equal(turn.model, "claude-sonnet-4-5");
assert.equal(turn.content, "", "a new turn starts a fresh reply");
assert.equal(state().messages.filter(m => m.role === "assistant").length, 2);
state().event("turn", schema.StreamDataSchema.parse({ provider_kind: "codex_cli" }));
assert.equal(state().messages.at(-1).model, "", "a turn without a model never infers the selected one");
state().event("status", schema.StreamDataSchema.parse({ model: "gpt-5" }));
assert.equal(state().messages.at(-1).model, "gpt-5");
state().reset();
assert.deepEqual(state().messages, [], "reset clears the chart source");
assert.equal(state().settings.model, "selected-model", "reset keeps the selected model for the next chat");

// --- The enabled Dashboard panel actually mounts the chart (regression guard for an empty aside).
const screen = fs.readFileSync(src("components/features/chat/chat-screen.tsx"), "utf8");
assert.match(screen, /import \{ TokenDashboard \} from "\.\/token-dashboard";/);
assert.match(screen, /<aside className=\{styles\.dashboard\}[^>]*><TokenDashboard \/><\/aside>/, "the aside must render the token chart");

// --- API request scenarios run last so the final PASS only prints after every awaited check.
async function main() {
  await apiRequestScenarios();
  console.log("PASS chat token usage: anchored reports, last snapshot, unknown models, zero counts, cache/reasoning exclusion, schema, API request scenarios, store stamping and dashboard mount");
}

main().catch(error => {
  console.error(error && error.stack ? error.stack : error);
  process.exitCode = 1;
});
