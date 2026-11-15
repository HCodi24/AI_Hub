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

// --- Rendered card: Vietnamese labels, exact numbers, textual equivalents and the honest empty state.
const React = fromWeb("react");
const { renderToStaticMarkup } = fromWeb("react-dom/server");
const chatText = load(src("text/chat.ts")).chatText;
const { TokenDashboardView } = load(src("components/features/chat/token-dashboard.tsx"), {
  "@/text/chat": { chatText }, "./chat-store": { useChatStore: () => [] },
});
const render = list => renderToStaticMarkup(React.createElement(TokenDashboardView, { messages: list }));
const emptyHtml = render([]);
assert.ok(emptyHtml.includes("Token phiên chat"), "title is always shown");
assert.ok(emptyHtml.includes("Chưa có dữ liệu token."), "honest empty state");
assert.ok(!emptyHtml.includes("Tổng đã ghi"), "no fabricated totals without reports");
const cardHtml = render(messages);
for (const wanted of ["Token phiên chat", "Tổng đã ghi: vào 1.264, ra 54", ">Vào<", ">Ra<", "gpt-5", "Không rõ model",
  "Vào 1.212", "Ra 48", "gpt-5: vào 1.212, ra 48", "Không rõ model: vào 45, ra 3", "không phải tổng chi phí đầy đủ"])
  assert.ok(cardHtml.includes(wanted), `missing ${wanted}`);
assert.ok(!cardHtml.includes("no-report"), "messages without a report never appear");
assert.equal((cardHtml.match(/data-kind="input"/g) || []).length, 7, "legend swatch plus one segment and one count per model");
assert.equal((cardHtml.match(/data-kind="output"/g) || []).length, 7, "output is striped, never colour-only");

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

console.log("PASS chat token usage: anchored reports, last snapshot, unknown models, zero counts, cache/reasoning exclusion, schema, store stamping and dashboard mount");
