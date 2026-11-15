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
const { chatTokenUsage, chatTokenSeries, lastTokenReport, formatTokens } = load(src("lib/chat-token-usage.ts"));
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

// --- Cumulative series: shared zero origin, reply order, carry-forward and exact endpoints.
const seriesOf = list => chatTokenSeries(list);
const pointsOf = (series, model) => series.series.find(entry => entry.model === model).points;
const totalsOf = (series, model) => pointsOf(series, model).map(point => point.total);

// Empty chat: no series, no steps, a finite zero maximum.
assert.deepEqual(seriesOf([]), { models: [], steps: 0, series: [], max: 0 });
assert.deepEqual(seriesOf([user(["Token: vào 1, ra 1"])]), { models: [], steps: 0, series: [], max: 0 },
  "user logs never create a step");

// One report: step 0 is zero, step 1 is the report, and the endpoint equals chatTokenUsage.
const singleSeries = seriesOf([assistant("gpt-5", ["Token: vào 100, ra 10"])]);
assert.equal(singleSeries.steps, 1);
assert.deepEqual(pointsOf(singleSeries, "gpt-5"), [
  { step: 0, input: 0, output: 0, total: 0 },
  { step: 1, input: 100, output: 10, total: 110 },
]);
assert.equal(singleSeries.max, 110);

// Interleaved replies: every model starts at step 0, others carry forward, endpoints match the aggregation.
const interleaved = seriesOf(messages);
assert.equal(interleaved.steps, 5, "one step per reported assistant reply");
assert.deepEqual(interleaved.models, ["gpt-5", "claude-sonnet-4-5", ""], "models appear in first-report order");
assert.deepEqual(totalsOf(interleaved, "gpt-5"), [0, 1245, 1245, 1260, 1260, 1260]);
assert.deepEqual(totalsOf(interleaved, "claude-sonnet-4-5"), [0, 0, 10, 10, 10, 10], "a late model stays at zero before its first report");
assert.deepEqual(totalsOf(interleaved, ""), [0, 0, 0, 0, 42, 48], "the unknown model is its own series");
for (const entry of interleaved.series) {
  assert.deepEqual(entry.points[0], { step: 0, input: 0, output: 0, total: 0 }, "shared zero origin");
  assert.equal(entry.points.length, interleaved.steps + 1, "one point per step plus the origin");
  const exact = usage.models.find(group => group.model === entry.model);
  const last = entry.points.at(-1);
  assert.deepEqual([last.input, last.output, last.total], [exact.input, exact.output, exact.total],
    `endpoint of ${entry.model || "unknown"} equals chatTokenUsage`);
  assert.equal(last.total, last.input + last.output);
}
assert.equal(interleaved.max, 1260, "the maximum is the largest cumulative total");

// Repeated logs in one message are replacement reports, never summed.
const repeated = seriesOf([assistant("m", ["Token: vào 100, ra 10", "Token: vào 1200 (cache 300), ra 45 (suy luận 10)"])]);
assert.equal(repeated.steps, 1, "one step per message, not per log line");
assert.deepEqual(totalsOf(repeated, "m"), [0, 1245]);
assert.deepEqual(chatTokenSeries([assistant("m", ["Token: vào 100, ra 10", "Token: vào x, ra 3"])]).series[0].points.at(-1),
  { step: 1, input: 100, output: 10, total: 110 }, "a malformed tail falls back to the last valid report");

// Zero and unknown models stay explicit series; invalid and user logs are excluded.
const zeroSeries = seriesOf([assistant("m-zero", ["Token: vào 0, ra 0"])]);
assert.deepEqual(zeroSeries.models, ["m-zero"]);
assert.deepEqual(totalsOf(zeroSeries, "m-zero"), [0, 0], "an explicit zero report still creates a step");
assert.equal(zeroSeries.steps, 1);
const unknownSeries = seriesOf([assistant("", ["Token: vào 40, ra 2"])]);
assert.deepEqual(unknownSeries.models, [""], "the unknown model keeps its empty identity");
assert.deepEqual(totalsOf(unknownSeries, ""), [0, 42]);
const excluded = seriesOf([
  user(["Token: vào 999, ra 999"]),
  assistant("no-report", []),
  assistant("bad-report", ["Token: vào x, ra y"]),
  assistant("m", ["Token: vào 5, ra 5"]),
]);
assert.deepEqual(excluded.models, ["m"], "only assistant messages with a valid report create series");
assert.equal(excluded.steps, 1);
assert.deepEqual(totalsOf(excluded, "m"), [0, 10]);

// The helper never mutates the messages or the logs it reads.
const frozenLogs = ["Token: vào 3, ra 4"];
const frozen = [assistant("m", frozenLogs)];
chatTokenSeries(frozen);
assert.deepEqual(frozenLogs, ["Token: vào 3, ra 4"], "original logs are untouched");
assert.deepEqual(frozen[0].logs, ["Token: vào 3, ra 4"]);

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
const render = (list, samples = [], status = "ready", compact = false) => renderToStaticMarkup(React.createElement(TokenDashboardView, { messages: list, samples, status, compact }));
const emptyHtml = render([]);
assert.ok(emptyHtml.includes("Token phiên chat"), "title is always shown");
assert.ok(emptyHtml.includes("Chưa có dữ liệu token."), "honest empty state");
assert.ok(!emptyHtml.includes("Tổng đã ghi"), "no fabricated totals without reports");
assert.ok(emptyHtml.includes(chatText.tokenDashboard.historyEmpty), "ready + empty history is reported as empty");
assert.ok(!emptyHtml.includes(chatText.tokenDashboard.waiting), "a ready empty history is not loading");
assert.ok(!emptyHtml.includes('<svg'), "no chart without reports");
// Loading, ready-empty and error stay distinct even when the messages carry no reports.
const emptyLoading = render([], [], "loading");
assert.ok(emptyLoading.includes(chatText.tokenDashboard.waiting), "loading is shown without reports");
assert.ok(!emptyLoading.includes(chatText.tokenDashboard.historyEmpty), "loading is not reported as empty");
assert.ok(!emptyLoading.includes(chatText.tokenDashboard.historyError), "loading is not reported as an error");
assert.ok(!emptyLoading.includes('<svg'), "no chart while loading without reports");
const emptyError = render([], [], "error");
assert.ok(emptyError.includes(chatText.tokenDashboard.historyError), "a failure is shown without reports");
assert.ok(!emptyError.includes(chatText.tokenDashboard.waiting), "a failure is not reported as loading");
assert.ok(!emptyError.includes(chatText.tokenDashboard.historyEmpty), "a failure is not reported as empty");
assert.ok(!emptyError.includes('<svg'), "no chart without reports");
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
// Main chart: one end dot per model = 3; backend chart: 2 samples x 3 models = 6.
assert.equal((cardHtml.match(/<circle/g) || []).length, 9);
assert.ok(!/NaN|Infinity/.test(cardHtml));
assert.match(cardHtml, /<table/);
const single = render(messages, [baseline]);
assert.equal((single.match(/<circle/g) || []).length, 6, "3 main end dots plus 3 backend points");
assert.ok(!/NaN|Infinity/.test(single));
const zeroHtml = render([assistant("m-zero", ["Token: vào 0, ra 0"])], [{at: baseline.at, models: [{model:"m-zero",input:0,output:0,total:0}]}]);
assert.equal((zeroHtml.match(/<circle/g) || []).length, 2, "1 main end dot plus 1 backend point");
assert.ok(!/NaN|Infinity/.test(zeroHtml));
const waiting = render(messages, [], "loading");
assert.ok(waiting.includes(chatText.tokenDashboard.waiting));
assert.match(waiting, /<svg/, "the main chart renders while the history loads");
assert.ok(!/NaN|Infinity/.test(waiting));
const late = render(messages, [baseline, {at: next.at, models: [...next.models, {model: "late", input:1,output:1,total:2}]}]);
assert.equal((late.match(/<circle/g) || []).length, 10, '3 main end dots plus 7 backend points; the late model has no invented earlier point');
const collision = render(messages, [{at:baseline.at, models:[{model:"",input:0,output:0,total:0},{model:"unknown",input:1,output:0,total:1}]}]);
assert.equal((collision.match(/<circle/g) || []).length, 5, "3 main end dots plus 2 backend points");

// --- Local reports with an empty backend history: the main cumulative chart is drawn from the logs.
const localOnly = render(messages);
assert.ok(localOnly.includes(chatText.tokenDashboard.total("1.264", "54")), "exact totals come from the chat logs");
assert.ok(localOnly.includes(chatText.tokenDashboard.cumulativeTitle), "the main chart is always shown when reports exist");
assert.ok(localOnly.includes(chatText.tokenDashboard.historyUnavailable), "the missing timestamped history is explained");
assert.ok(!localOnly.includes(chatText.tokenDashboard.historyEmpty), "no false claim that the backend recorded nothing");
assert.ok(!localOnly.includes(chatText.tokenDashboard.waiting), "a ready empty history is not loading");
assert.ok(!localOnly.includes(chatText.tokenDashboard.historyError), "a ready empty history is not an error");
assert.match(localOnly, /<svg/, "the main chart renders without server samples");
assert.match(localOnly, /d="M[^"]*H[^"]*V/, "the series are drawn as stepped lines");
assert.ok(!/NaN|Infinity/.test(localOnly), "no invalid coordinates without samples");
assert.ok(localOnly.includes(chatText.tokenDashboard.replyAxis), "the x axis is labelled with the reply order");
assert.ok(localOnly.includes(chatText.tokenDashboard.cumulativeAxis), "the y axis is labelled with cumulative tokens");
assert.ok(localOnly.includes(chatText.tokenDashboard.cumulativeDesc), "the accessible description explains the accumulation");
assert.equal((localOnly.match(/<circle/g) || []).length, 3, "one end dot per model");
assert.ok(localOnly.includes('M42 175'), "every series starts at the shared zero baseline");
// The exact table is collapsed below the main chart and keeps the exact numbers.
assert.match(localOnly, /<details[^>]*><summary>Xem bảng số liệu chính xác<\/summary>/);
for (const rowHtml of [
  '<th scope="row">gpt-5</th><td>1.212</td><td>48</td><td>1.260</td>',
  '<th scope="row">Không rõ model</th><td>45</td><td>3</td><td>48</td>',
  '<th scope="row">claude-sonnet-4-5</th><td>7</td><td>3</td><td>10</td>',
]) assert.ok(localOnly.includes(rowHtml), 'missing exact row ' + rowHtml);
assert.ok(!localOnly.includes("no-report") && !localOnly.includes("bad-report"), "no fabricated model rows");
// The legend carries the full model names and the current totals.
assert.ok(localOnly.includes('class="legendName" title="gpt-5">gpt-5<'), "the legend keeps the full model name");
assert.ok(localOnly.includes('class="legendValue">1,3K<'), "the legend shows the current total");
assert.ok(localOnly.includes('class="legendValue">48<'));
assert.ok(localOnly.includes('class="legendValue">10<'));
// No leftover horizontal bars.
assert.ok(!localOnly.includes('class="barValue"') && !localOnly.includes('class="barTrack"'), "the bars are gone");

// --- All-zero counts: the chart still renders finite coordinates.
const zeroLocal = render([assistant("m-zero", ["Token: vào 0, ra 0"])]);
assert.ok(zeroLocal.includes(chatText.tokenDashboard.total("0", "0")));
assert.ok(zeroLocal.includes('<th scope="row">m-zero</th><td>0</td><td>0</td><td>0</td>'));
assert.match(zeroLocal, /<svg/);
assert.equal((zeroLocal.match(/<circle/g) || []).length, 1, "the single zero total gets its end dot");
assert.ok(!/NaN|Infinity/.test(zeroLocal));
assert.ok(zeroLocal.includes(chatText.tokenDashboard.historyUnavailable), "zero reports still explain the missing history");

// --- Unknown model and a long model name render safely inside the narrow sidebar.
const unknownLocal = render([assistant("", ["Token: vào 40, ra 2"])]);
assert.ok(unknownLocal.includes(chatText.tokenDashboard.unknownModel));
assert.ok(unknownLocal.includes('<th scope="row">Không rõ model</th><td>40</td><td>2</td><td>42</td>'));
assert.ok(!/NaN|Infinity/.test(unknownLocal));
const longModel = "provider/very-long-model-name-with-many-segments-and-a-suffix-2025-11";
const longLocal = render([assistant(longModel, ["Token: vào 9, ra 1"])]);
assert.ok(longLocal.includes(longModel), "the exact long model name is kept");
assert.ok(longLocal.includes(`title="${longModel}"`), "the legend keeps the full name as a tooltip");
assert.ok(!/NaN|Infinity/.test(longLocal));

// --- No reports at all: no chart and no table, only the honest empty state.
assert.ok(!emptyHtml.includes(chatText.tokenDashboard.cumulativeTitle), "no chart without reports");
assert.ok(!emptyHtml.includes("<table"), "no table without reports");

// --- A populated server timeline keeps its own collapsed chart next to the main one.
assert.ok(cardHtml.includes(chatText.tokenDashboard.cumulativeTitle), "the main chart coexists with the server history");
assert.ok(cardHtml.includes(chatText.tokenDashboard.serverDetails), "the backend history is a labelled secondary section");
assert.ok(cardHtml.includes(chatText.tokenDashboard.timeline), "the server timeline caption is preserved");
assert.ok(!cardHtml.includes(chatText.tokenDashboard.historyUnavailable), "no missing-history note when samples exist");
assert.ok(cardHtml.includes('<th scope="row">gpt-5</th><td>1.212</td><td>48</td><td>1.260</td>'),
  "the cumulative table is independent of the sampled timeline");
// The main chart comes first; the backend chart lives inside the collapsed details.
assert.ok(cardHtml.indexOf(chatText.tokenDashboard.cumulativeTitle) < cardHtml.indexOf(chatText.tokenDashboard.serverDetails),
  "the main chart is the prominent one");
assert.ok(cardHtml.indexOf(chatText.tokenDashboard.serverDetails) < cardHtml.indexOf(chatText.tokenDashboard.timeline),
  "the backend chart is inside its own details section");

// --- Loading and error with usable local reports: the main chart stays visible and finite.
const loadingLocal = render(messages, [], "loading");
assert.ok(loadingLocal.includes(chatText.tokenDashboard.waiting), "the loading note is kept");
assert.match(loadingLocal, /<svg/, "the main chart renders while the history loads");
assert.ok(!/NaN|Infinity/.test(loadingLocal));
const errorLocal = render(messages, [], "error");
assert.ok(errorLocal.includes(chatText.tokenDashboard.historyError), "the error note is kept");
assert.match(errorLocal, /<svg/, "the main chart renders after a history failure");
assert.ok(!/NaN|Infinity/.test(errorLocal));
assert.equal((errorLocal.match(new RegExp(chatText.tokenDashboard.historyError, "g")) || []).length, 1, "one error note only");
// The history notes live inside the collapsed backend section, never above the main chart.
assert.ok(errorLocal.indexOf(chatText.tokenDashboard.cumulativeTitle) < errorLocal.indexOf(chatText.tokenDashboard.historyError),
  "the note does not precede the main chart");

// --- Compact frame: at-a-glance summary (total, input/output split, model ranking, per-reply columns), no notes, tables or history.
const compactHtml = render(messages, [baseline, next], "ready", true);
assert.match(compactHtml, /<svg/, "the compact frame keeps a per-reply chart");
assert.ok(compactHtml.includes(chatText.tokenDashboard.cumulativeTitle), "the compact chart is the cumulative per-model chart");
assert.equal((compactHtml.match(/<circle/g) || []).length, 3, "one end dot per model");
assert.ok(compactHtml.includes('class="legendName" title="gpt-5">gpt-5<'), "the compact frame keeps the model ranking");
assert.ok(compactHtml.includes(chatText.dashboard.title), "the compact frame keeps a short Dashboard heading");
assert.ok(!compactHtml.includes(chatText.tokenDashboard.cumulativeNote), "no explanation paragraph in the compact frame");
assert.ok(!compactHtml.includes(chatText.tokenDashboard.note), "no note in the compact frame");
assert.ok(!compactHtml.includes(chatText.tokenDashboard.exactDetails), "no exact table control in the compact frame");
assert.ok(!compactHtml.includes(chatText.tokenDashboard.serverDetails), "no backend history control in the compact frame");
assert.ok(!compactHtml.includes(chatText.tokenDashboard.recording), "no backend recording note in the compact frame");
assert.ok(!compactHtml.includes("<table"), "no table in the compact frame");
assert.ok(!/NaN|Infinity/.test(compactHtml), "no invalid coordinates in the compact frame");
const compactEmpty = render([], [baseline, next], "ready", true);
assert.ok(compactEmpty.includes(chatText.tokenDashboard.empty), "the compact frame keeps the honest empty state");
assert.ok(!compactEmpty.includes("<svg"), "no chart without reports in the compact frame");
assert.ok(!compactEmpty.includes(chatText.tokenDashboard.serverDetails), "no backend history in the compact empty frame");
// The full view keeps every supporting section the compact frame hides.
assert.ok(cardHtml.includes(chatText.tokenDashboard.total("1.264", "54")), "the full view keeps the totals");
assert.ok(cardHtml.includes(chatText.tokenDashboard.cumulativeNote), "the full view keeps the explanation paragraph");
assert.ok(cardHtml.includes(chatText.tokenDashboard.exactDetails), "the full view keeps the exact table control");
assert.ok(cardHtml.includes(chatText.tokenDashboard.serverDetails), "the full view keeps the backend history control");
assert.ok(cardHtml.includes(chatText.tokenDashboard.recording), "the full view keeps the backend recording note");
assert.ok(cardHtml.includes('class="modelsTitle"'), "the full view keeps the cumulative section heading");

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

// --- The enabled Dashboard frame mounts the compact chart, and the large dialog mounts the full one.
const screen = fs.readFileSync(src("components/features/chat/chat-screen.tsx"), "utf8");
assert.match(screen, /import \{ TokenDashboard \} from "\.\/token-dashboard";/);
assert.match(screen, /<aside className=\{styles\.dashboard\}[^>]*><TokenDashboard compact \/><\/aside>/,
  "the aside must render the compact token chart");
assert.ok(!/<aside className=\{styles\.dashboard\}[^>]*><TokenDashboard \/>/.test(screen),
  "the frame never mounts the full dashboard with its history polling");
const dialog = fs.readFileSync(src("components/features/chat/dashboard-dialog.tsx"), "utf8");
assert.match(dialog, /import \{ TokenDashboard \} from "\.\/token-dashboard";/);
assert.match(dialog, /<div className=\{styles\.body\}><TokenDashboard \/><\/div>/,
  "the dialog body must mount the full token dashboard");
assert.equal((dialog.match(/<TokenDashboard \/>/g) || []).length, 1, "exactly one full dashboard instance");
assert.ok(!/blank/i.test(dialog), "no stale blank-body description");
const dialogCss = fs.readFileSync(src("components/features/chat/dashboard-dialog.module.css"), "utf8");
assert.match(dialogCss, /\.body \{[^}]*overflow-y:\s*auto/, "the dialog body scrolls vertically");
assert.match(dialogCss, /\.body \{[^}]*background:\s*#fff/, "the dialog body keeps a white background");
assert.ok(!/blank/i.test(dialogCss), "no stale blank-body comment in the dialog styles");

// --- Dashboard frame stacking: the visible gutter card must sit above the full-width message
// scroller inside an isolated workspace, otherwise the chat scroller swallows the click and the
// blank dialog never opens. These assertions guard the CSS contract that makes the click reachable.
const screenCss = fs.readFileSync(src("components/features/chat/chat-screen.module.css"), "utf8");
const cssRule = selector => {
  const match = screenCss.match(new RegExp(`(?:^|\\n)\\.${selector}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `missing .${selector} rule`);
  return match[1];
};
const workspaceRule = cssRule("workspace");
assert.match(workspaceRule, /position:\s*relative/, "the workspace anchors the absolutely positioned card");
assert.match(workspaceRule, /isolation:\s*isolate/, "the workspace isolates the local stacking levels from dialogs and toasts");
const dashboardRule = cssRule("dashboard");
assert.match(dashboardRule, /position:\s*absolute/, "the card stays out of flow in the gutter");
assert.match(dashboardRule, /z-index:\s*1\b/, "the card is explicitly above the message scroller");
assert.match(dashboardRule, /width:\s*min\(240px/, "the gutter width is preserved");
const messagesRule = cssRule("messages");
assert.match(messagesRule, /position:\s*relative/, "the scroller needs a position for its local z-index");
assert.match(messagesRule, /z-index:\s*0\b/, "the scroller is explicitly below the card");
assert.match(messagesRule, /overflow:\s*hidden/, "the scroller keeps its own scrolling");
assert.ok(!/pointer-events:\s*none/.test(screenCss), "the chat never disables pointer events");
assert.match(screenCss, /@container \(min-width: 1040px\) \{\s*\.dashboard \{ display: block; \}/,
  "the responsive Dashboard visibility is preserved");
assert.match(screenCss, /@media \(min-width: 768px\)/, "the responsive shell layout is preserved");

// --- The frame handlers stay wired: plain text, SVG and padding open the blank dialog, while
// nested interactive descendants (details/summary, buttons, links, inputs) keep their behaviour.
assert.match(screen, /const \[dashboardOpen, setDashboardOpen\] = useState\(false\)/);
assert.match(screen, /onClick=\{onDashboardClick\}/, "the frame keeps its click handler");
assert.match(screen, /onKeyDown=\{onDashboardKeyDown\}/, "the frame keeps its keyboard handler");
assert.match(screen, /const INTERACTIVE = "[^"]*summary[^"]*details/, "details/summary stay interactive");
assert.match(screen, /if \(hit && hit !== event\.currentTarget\) return;/, "interactive descendants are exempted");
assert.match(screen, /setDashboardOpen\(true\)/, "the frame opens the blank dialog");
assert.match(screen, /\{dashboardOpen && <DashboardDialog close=\{\(\) => setDashboardOpen\(false\)\} \/>\}/,
  "the blank dialog is rendered conditionally and can be closed");

// --- API request scenarios run last so the final PASS only prints after every awaited check.
async function main() {
  await apiRequestScenarios();
  console.log("PASS chat token usage: anchored reports, last snapshot, cumulative reply-order series (shared zero origin, late/interleaved models, carry-forward, endpoints), unknown models, zero counts, cache/reasoning exclusion, schema, API request scenarios, store stamping, dashboard mount and the isolated workspace stacking contract (card z-index 1 above the scroller z-index 0, handlers and blank dialog wired)");
}

main().catch(error => {
  console.error(error && error.stack ? error.stack : error);
  process.exitCode = 1;
});
