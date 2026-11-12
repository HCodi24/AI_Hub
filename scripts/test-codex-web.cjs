const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");
const web = path.resolve(__dirname, "../web");
const fromWeb = createRequire(path.join(web, "package.json"));
const ts = fromWeb("typescript");
const React = fromWeb("react");
const { renderToStaticMarkup } = fromWeb("react-dom/server");
const { QueryClient, QueryClientProvider } = fromWeb("@tanstack/react-query");
const cache = new Map();
function load(file) {
  if (cache.has(file)) return cache.get(file);
  const module = { exports: {} };
  const source = ts.transpileModule(fs.readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const requireLocal = id => {
    if (id.endsWith(".module.css")) return { default: new Proxy({}, { get: (_, key) => key }) };
    if (id === "@/components/button/button") return { Button: ({ variant, ...props }) => React.createElement("button", props) };
    if (id.startsWith("@/")) return load(path.join(web, "src", id.slice(2) + ".ts"));
    return fromWeb(id);
  };
  vm.runInThisContext(`(function(require,module,exports){${source}\n})`, { filename: file })(requireLocal, module, module.exports);
  cache.set(file, module.exports);
  return module.exports;
}
async function run() {
  const { codexApi, safeCodexLoginUrl } = load(path.join(web, "src/apis/codex.api.ts"));
  assert.ok(safeCodexLoginUrl("https://auth.openai.com/oauth/authorize"));
  for (const url of ["javascript:alert(1)", "https://auth.openai.com.evil.test", "http://chatgpt.com", "https://user@chatgpt.com"]) assert.equal(safeCodexLoginUrl(url), null);
  const state = { connected: false, pending: false, message: "Chưa đăng nhập", project_skills: false, project_instructions: false };
  const { http } = load(path.join(web, "src/lib/http.ts"));
  const requests = [];
  http.request = async (url, init) => { requests.push({ url, init }); return { json: async () => state }; };
  await codexApi.action("codex/id", "web");
  assert.equal(requests[0].url, "/providers/codex%2Fid/codex");
  assert.deepEqual(JSON.parse(requests[0].init.body), { action: "web" });
  await codexApi.settings("codex/id", { project_skills: true, project_instructions: false });
  assert.equal(requests[1].init.method, "PUT");
  assert.deepEqual(JSON.parse(requests[1].init.body), { project_skills: true, project_instructions: false });
  const { CodexConnection } = load(path.join(web, "src/components/features/settings/codex-connection.tsx"));
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  const render = data => {
    client.setQueryData(["codex-connection", "codex"], data);
    return renderToStaticMarkup(React.createElement(QueryClientProvider, { client }, React.createElement(CodexConnection, { providerId: "codex" })));
  };
  const initial = render(state);
  assert.ok(initial.includes("Tìm Codex CLI") && initial.includes("Đăng nhập qua web"));
  assert.equal((initial.match(/type="checkbox"/g) || []).length, 2);
  assert.ok(!initial.includes(" checked="));
  assert.ok(!initial.includes("<form"), "Connection section must not nest a form inside ProviderForm");
  const pending = render({ ...state, pending: true, url: "https://auth.openai.com/oauth/authorize" });
  assert.ok(pending.includes("Hủy đăng nhập") && pending.includes('rel="noopener noreferrer"'));
  assert.ok(!render({ ...state, pending: true, url: "javascript:alert(1)" }).includes("javascript:"));
  assert.ok(render({ ...state, connected: true }).includes("Đăng xuất khỏi MutiS"));
  client.clear();
  console.log("PASS Codex web: safe browser URLs, encoded API requests, independent switches, connection states and non-nested UI");
}
run().catch(error => { console.error(error); process.exitCode = 1; });
