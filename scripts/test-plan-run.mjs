import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";

const node = { replaceChildren() {}, append() {}, querySelector() { return null; } };
const state = { currentPlan: null, plans: [], mutis: { enabled: false }, streaming: false };
let failId = 2;
let stopId = null;
let verifyFailures = 0;
const started = [];
const saved = [];
const progress = [];
const provider = { id: "fake", models: ["fake"], kind: "openai_compat", agent_capable: true };
let runner;
const api = async (url, options = {}) => {
  if (options.method === "PUT") { saved.push(JSON.parse(options.body)); return {}; }
  if (url === "/api/plans") return [];
  if (url.endsWith("/stop")) return {};
  if (url === "/api/mutis/roles/resolve" || url === "/api/mutis/events") return {};
  if (url === "/api/verify") return { body: { verify: true } };
  if (url === "/api/chat") {
    const body = JSON.parse(options.body);
    started.push(Number(body.task_id));
    assert.equal(body.file_tools, "write");
    return { body };
  }
  throw new Error(`Unexpected mock request ${url}`);
};
const readSse = async (body, callback) => {
  if (body.verify) {
    callback("verify", { ok: verifyFailures-- <= 0, summary_lines: ["Kiểm tra giả lập"] });
    return;
  }
  if (Number(body.task_id) === stopId) { await runner.stopWholePlan(); return; }
  if (Number(body.task_id) === failId) callback("error", { text: "Lỗi giả lập" });
  else callback("delta", { text: "Xong giả lập" });
};
globalThis.crypto ||= webcrypto;
globalThis.window = { dispatchEvent() {} };
globalThis.CustomEvent = class { constructor(name, options) { this.type = name; this.detail = options?.detail; } };
// Replace only imports with local fakes. The runner, persistence and task state machine remain the shipped source.
globalThis.planRunTest = {
  state, api, readSse, $: () => node, el: () => node, showToast() {},
  hideMemory: text => text, memoryEvent() {}, EFFORTS: [], findProvider: () => provider,
  folderInput: { value: "project" }, devChatPreamble: () => "",
};
const helpers = await readFile(new URL("../ui/js/plan-helpers.js", import.meta.url), "utf8");
const order = await readFile(new URL("../ui/js/plan-order.js", import.meta.url), "utf8");
let source = await readFile(new URL("../ui/js/plans.js", import.meta.url), "utf8");
source = source.replace(/^import .*;\r?\n/gm, "").replace(/^export \{ (?:planRunOrder|hasCycle) \}.*;\r?\n/gm, "");
source = `const {state,api,readSse,$,el,showToast,hideMemory,memoryEvent,EFFORTS,findProvider,folderInput,devChatPreamble} = globalThis.planRunTest;\n${helpers}\n${order}\n${source}`;
runner = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const task = (id, depends_on = [], status = "chờ") => ({
  id, depends_on, status, title: `Task ${id}`, detail: "", assignee: "fake", kind: "code", log: [],
});
const plan = { id: "mock", approved: true, title: "Mock", folder: "project", auto_continue: true,
  tasks: [task(3, [2]), task(1), task(2, [1])] };
await runner.runWholePlan(plan, p => progress.push(p.tasks.map(t => t.status)));
assert.deepEqual(started, [1, 2]);
assert.equal(plan.tasks[2].status, "lỗi");
assert.equal(plan.tasks[2].log.at(-1), "Lỗi: Lỗi giả lập");
assert.equal(runner.activePlanRun, null);
failId = null;
await runner.runWholePlan(plan);
assert.deepEqual(started, [1, 2, 2, 3]);
assert.ok(plan.tasks.every(t => t.status === "xong"));
assert.ok(progress.some(statuses => statuses.includes("đang chạy")));
assert.ok(saved.every(p => !("_running" in p)));
stopId = 4;
const cancelled = { ...plan, tasks: [task(4), task(5, [4])] };
await runner.runWholePlan(cancelled);
assert.equal(cancelled.tasks[0].status, "đã dừng");
assert.equal(cancelled.tasks[1].status, "chờ");
assert.equal(runner.activePlanRun, null);
await assert.rejects(() => runner.runWholePlan({ ...plan, approved: false }), /chưa được duyệt/u);
stopId = null;
state.mutis = { enabled: true, verify_gate: true, auto_fix: true, fix_rounds_max: 2 };
verifyFailures = 1;
const fixed = { ...plan, tasks: [task(10)] };
await runner.runWholePlan(fixed);
assert.equal(fixed.tasks.length, 2);
assert.equal(fixed.tasks[1].assignee, fixed.tasks[0].assignee);
assert.ok(fixed.tasks.every(t => t.status === "xong"));
verifyFailures = 10;
const failedGate = { ...plan, tasks: [task(20), task(21, [20])] };
state.mutis.auto_fix = false;
await runner.runWholePlan(failedGate);
assert.equal(failedGate.tasks[0].status, "lỗi");
assert.equal(failedGate.tasks[1].status, "chờ");
console.log("runWholePlan: order, failure, resume, progress, persistence, cancellation, verify gate and auto-fix passed (local fakes)");
