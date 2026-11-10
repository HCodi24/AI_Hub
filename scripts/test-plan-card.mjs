import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ui = fileURLToPath(new URL("../ui/", import.meta.url));
const modules = new Map();
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) await collect(file);
    else if (file.endsWith(".js")) {
      modules.set(file, new vm.SourceTextModule(await readFile(file, "utf8"), { identifier: file }));
    }
  }
}
await collect(ui);
// Link the actual entry point, including cycles, to catch missing imports/exports that node --check misses.
await modules.get(path.join(ui, "js/main.js")).link((specifier, parent) => {
  const file = specifier.startsWith("/") ? path.join(ui, specifier.slice(1)) : path.resolve(path.dirname(parent.identifier), specifier);
  assert.ok(modules.has(file), `Missing module ${file}`);
  return modules.get(file);
});

class Node {
  constructor(tag) { this.tag = tag; this.children = []; this.value = ""; this.disabled = false; }
  append(...items) { this.children.push(...items); }
  prepend(...items) { this.children.unshift(...items); }
  replaceChildren(...items) { this.children = items; }
  set textContent(text) { this.children = [text]; }
  get textContent() { return this.children.map(child => typeof child === "string" ? child : child.textContent).join(""); }
  querySelectorAll(selector) {
    const matches = [];
    for (const child of this.children) if (child instanceof Node) {
      if (selector.split(/,\s*/).includes(child.tag)) matches.push(child);
      matches.push(...child.querySelectorAll(selector));
    }
    return matches;
  }
}
const events = new Map();
globalThis.window = { addEventListener(name, callback) { events.set(name, callback); } };
const provider = { id: "codex", name: "Codex", models: ["model-a", "model-b"] };
const state = { config: { providers: [provider] }, providerStatuses: new Map(), messages: [], mutis: { roles: {
  lead: { provider_id: provider.id, model: "model-b", effort: "high" },
} } };
let saves = 0;
let runs = 0;
const el = (tag, props = {}, children = []) => Object.assign(new Node(tag), props, { children: props.textContent ? [props.textContent] : children });
const summarySource = await readFile(new URL("../ui/js/plan-summary.js", import.meta.url), "utf8");
const { planSummary } = await import(`data:text/javascript;base64,${Buffer.from(summarySource).toString("base64")}`);
const helperSource = await readFile(new URL("../ui/js/plan-helpers.js", import.meta.url), "utf8");
const { hasCycle, taskCanWrite } = await import(`data:text/javascript;base64,${Buffer.from(helperSource).toString("base64")}`);
assert.ok(hasCycle([{ id: 1, depends_on: [2] }, { id: 2, depends_on: [1] }]));
assert.ok(!hasCycle([{ id: 1, depends_on: [] }, { id: 2, depends_on: [1] }]));
assert.equal(taskCanWrite({ kind: "code", permission: "read" }), false);
assert.equal(taskCanWrite({ kind: "small_edit" }), true);
globalThis.cardTest = {
  el, state, planSummary, EFFORTS: ["default", "high"], findProvider: () => provider,
  api: async () => { throw new Error("Unexpected API request"); },
  showToast: text => { throw new Error(text); },
  taskCanWrite, hasCycle,
  persistPlan: async plan => { saves++; events.get("plan-progress")({ detail: plan }); },
  runWholePlan: async () => { runs++; }, runPlanTask: async () => { runs++; },
  stopPlanTask() {}, stopWholePlan() {}, verifyPlan() {}, runTaskGates() {},
};
let source = await readFile(new URL("../ui/js/plan-card.js", import.meta.url), "utf8");
source = source.replace(/^(?:import|export \{).*;\r?\n/gm, "");
source = `const {el,state,api,showToast,planSummary,EFFORTS,findProvider,hasCycle,taskCanWrite,persistPlan,
runWholePlan,runPlanTask,stopPlanTask,stopWholePlan,verifyPlan,runTaskGates} = globalThis.cardTest;\n${source}`;
const { createPlanCard, storedPlanCard } = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const plan = { id: "fake", title: "Kế hoạch thử", request: "Thử UI", tasks: [{
  id: 1, title: "Giữ tiếng Việt", detail: "Chi tiết", assignee: "codex", kind: "code", status: "chờ", depends_on: [],
}] };
const card = createPlanCard();
card.load(plan);
assert.equal(storedPlanCard(plan.id), card);
const buttons = () => card.el.querySelectorAll("button");
const button = text => buttons().find(node => node.textContent === text);
const click = async text => { const node = button(text); assert.ok(node && !node.disabled, `Enabled button ${text}`); await node.onclick(); };
assert.ok(card.el.textContent.includes("Bản nháp"));
assert.ok(button("▶ Chạy task này").disabled);
await click("Duyệt kế hoạch");
assert.equal(plan.approved, true);
assert.equal(saves, 1);
await click("▶ Chạy tất cả");
assert.equal(runs, 0, "Whole plan requires inline confirmation");
await click("Chạy");
assert.equal(runs, 1);
state.planRunning = 1;
events.get("debate-busy")();
assert.ok(button("▶ Chạy task này").disabled);
state.planRunning = 0;
events.get("debate-busy")();
assert.ok(!button("▶ Chạy task này").disabled, "Controls recover after runner completes");
plan.tasks[0].summary = "Kết quả đang stream";
plan.tasks[0].status = "lỗi";
card.update(plan);
assert.ok(card.el.textContent.includes("Kết quả đang stream"));
assert.ok(button("Chạy tiếp"));
plan.approved = false;
plan.tasks[0].depends_on = [1];
card.update(plan);
assert.ok(button("Duyệt kế hoạch").disabled, "Cycle blocks approval");
const chooser = card.el.querySelectorAll("select")[0];
chooser.value = "role:lead";
await chooser.onchange();
assert.equal(plan.tasks[0].model, "model-b");
assert.equal(plan.tasks[0].effort, "high");
const pending = createPlanCard();
pending.fail("Đã hủy");
assert.ok(pending.el.textContent.includes("Đã hủy"));
console.log("plan-card: module graph, cache, approval, confirmation, busy recovery, output, cycle and MutiS passed (local fakes)");
