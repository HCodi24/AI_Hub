import { hasCycle, taskCanWrite } from "./plan-helpers.js";
import { el, showToast } from "./util.js";
import { api } from "./api.js";
import { state } from "./state.js";
import { planSummary } from "./plan-summary.js";
export { planSummary } from "./plan-summary.js";
import { EFFORTS, findProvider } from "./mode-chip.js";
import { persistPlan, runPlanTask, runWholePlan, stopPlanTask, stopWholePlan, verifyPlan, runTaskGates } from "./plans.js";

export const planCards = new Map();

export function createPlanCard() {
  const root = el("section", { className: "plan-card" });
  let plan = null;
  let planning = true;
  let liveStream = false;
  let fetching = false;
  let polled = 0;
  let timer = null;
  let confirm = false;
  const started = Date.now();
  const expanded = new Set();
  const header = el("header");
  const title = el("strong", { textContent: "✦ Chia việc" });
  const status = el("span", { role: "status" });
  const cancel = el("button", { textContent: "Hủy", onclick: () => root.dispatchEvent(new Event("plan-cancel")) });
  const logs = el("pre");
  const journal = el("details", {}, [el("summary", { textContent: "Nhật ký" }), logs]);
  const tasks = el("div");
  const footer = el("footer");
  header.append(title, status, cancel);
  root.append(header, journal, tasks, footer);
  function tick() {
    if (!planning) return;
    if (plan?.id && !liveStream && !fetching && Date.now() - polled > 2000) {
      fetching = true; polled = Date.now();
      api(`/api/plans/${encodeURIComponent(plan.id)}`).then(value => {
        plan = value; planning = !!value.planning; render();
        if (!planning) { clearInterval(timer); timer = null; }
      }).catch(error => { logs.textContent += `${error.message}\n`; }).finally(() => fetching = false);
    }
    const seconds = Math.floor((Date.now() - started) / 1000);
    status.replaceChildren(el("span", { className: "spinner", ariaLabel: "Đang lập kế hoạch" }),
      ` Đang lập kế hoạch… ${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`);
  }
  function button(text, action, disabled = false) {
    return el("button", { textContent: text, disabled, onclick: async () => {
      try { await action(); } catch (error) { showToast(error.message); }
    } });
  }
  const busy = () => !!state.streaming || !!state.planRunning || !!state.activeDebates?.size || !!plan?._running;
  async function save() {
    confirm = false;
    plan.approved = false;
    try { await persistPlan(plan); } catch (error) { showToast(error.message); }
  }
  function field(label, control) { return el("label", {}, [el("span", { textContent: label }), control]); }
  function select(values, value, change) {
    const node = el("select");
    for (const [id, name] of values) node.append(el("option", { value: id, textContent: name }));
    node.value = value;
    node.onchange = async () => { change(node.value); await save(); render(); };
    return node;
  }
  function output(label, text, key) {
    const node = el("details", { open: expanded.has(key) }, [el("summary", { textContent: label }), el("pre", { textContent: text })]);
    node.ontoggle = () => node.open ? expanded.add(key) : expanded.delete(key);
    return node;
  }
  function taskRow(task, index) {
    const running = task.status === "đang chạy";
    const row = el("details", { className: "pc-task", open: expanded.has(task.id) });
    row.ontoggle = () => row.open ? expanded.add(task.id) : expanded.delete(task.id);
    const icon = { "xong": "✓", "đang chạy": "◐", "lỗi": "✗", "cần sửa": "✗" }[task.status] || "○";
    const summary = el("summary", {}, [el("span", { textContent: `${icon} ${task.title}` }),
      el("span", { className: "pc-chip", textContent: findProvider(task.assignee)?.name || task.assignee }),
      el("span", { className: "pc-chip", textContent: task.kind })]);
    for (const id of task.depends_on || []) summary.append(el("span", { className: "pc-chip", textContent: `↳ #${id}` }));
    const detail = el("div", { className: "pc-detail" });
    const titleInput = el("input", { value: task.title, onchange: async event => { task.title = event.target.value; await save(); render(); } });
    const textarea = el("textarea", { value: task.detail, rows: 4, onchange: async event => { task.detail = event.target.value; await save(); } });
    const providers = state.config.providers;
    const assignees = providers.map(p => [p.id, `${p.name}${state.providerStatuses.get(p.id)?.ok === false ? " · chưa kết nối" : ""}`]);
    const roles = Object.entries(state.mutis?.roles || {});
    for (const [name, role] of roles) if (role.provider_id) assignees.push([`role:${name}`, `MutiS · ${name}`]);
    const assignee = select(assignees, task.assignee, value => {
      const role = value.startsWith("role:") ? state.mutis.roles[value.slice(5)] : null;
      task.assignee = role?.provider_id || value;
      task.model = role?.model || findProvider(task.assignee)?.models[0] || "";
      task.effort = role?.effort || "default";
    });
    const provider = findProvider(task.assignee);
    const models = [...new Set([task.model, ...(provider?.models || [])].filter(Boolean))].map(value => [value, value]);
    const deps = el("input", { value: (task.depends_on || []).join(","), onchange: async event => {
      task.depends_on = event.target.value.split(",").map(s => s.trim()).filter(Boolean).map(Number);
      await save(); render();
    } });
    detail.append(field("Tên việc", titleInput), field("Chi tiết", textarea), field("Giao cho", assignee),
      field("Model", select(models, task.model || provider?.models[0], value => task.model = value)),
      field("Effort", select(EFFORTS.map(value => [value, value]), task.effort || "default", value => task.effort = value)),
      field("Phụ thuộc (mã, phân cách bằng dấu phẩy)", deps),
      field("Loại", select(["plan", "small_edit", "code", "test", "docs", "review"].map(v => [v, v]), task.kind, value => task.kind = value)),
      field("Quyền", select([["read", "Chỉ đọc"], ["write", "Được sửa file"]], task.permission ||
        (taskCanWrite(task) ? "write" : "read"), value => task.permission = value)));
    if (task.reassigned_from) detail.append(el("small", { textContent: `⚠ đã đổi từ ${task.reassigned_from} (chưa kết nối)` }));
    const ready = (task.depends_on || []).every(id => plan.tasks.find(t => t.id === id)?.status === "xong");
    detail.append(button("▶ Chạy task này", () => runPlanTask(task, plan), !plan.approved || busy() || !ready));
    if (running) detail.append(button("Dừng", () => stopPlanTask(task, plan)));
    if (plan.approved) detail.append(
      button("Đánh dấu xong", async () => { task.status = "xong"; await persistPlan(plan); render(); }, busy()),
      button("Review lại", () => runTaskGates(task, plan), busy()));
    detail.append(button("↑", async () => { [plan.tasks[index - 1], plan.tasks[index]] = [task, plan.tasks[index - 1]]; await save(); render(); }, index === 0 || busy()),
      button("↓", async () => { [plan.tasks[index + 1], plan.tasks[index]] = [task, plan.tasks[index + 1]]; await save(); render(); }, index === plan.tasks.length - 1 || busy()),
      button("Xóa task", async () => { plan.tasks.splice(index, 1); await save(); render(); }, busy()));
    for (const [label, text] of [["Kết quả", task.summary], ["Nhật ký task", task.log?.join("\n")], ["Thay đổi", task.diff],
      ["Xác minh", task.verify && JSON.stringify(task.verify, null, 2)], ["Review", task.review && JSON.stringify(task.review, null, 2)]]) {
      if (text) detail.append(output(label, text, `${task.id}:${label}`));
    }
    if (busy()) for (const control of detail.querySelectorAll("input, textarea, select")) control.disabled = true;
    row.append(summary, detail);
    return row;
  }
  function render() {
    if (!plan) return;
    title.textContent = `✦ Chia việc · ${plan.title}`;
    cancel.hidden = !planning;
    status.className = planning ? "pc-planning" : "";
    if (planning) { tick(); return; }
    const done = plan.tasks.filter(task => task.status === "xong").length;
    status.textContent = plan.error ? "Lỗi" : plan._running || plan.tasks.some(task => task.status === "đang chạy") ? `Đang chạy ${done}/${plan.tasks.length}`
      : plan.tasks.some(task => ["lỗi", "cần sửa"].includes(task.status)) || !plan.tasks.length ? "Lỗi"
        : done === plan.tasks.length ? "Xong" : plan.approved ? "Đã duyệt" : "Bản nháp";
    tasks.replaceChildren(...plan.tasks.map(taskRow));
    if (plan.error) tasks.prepend(el("p", { className: "dc-error", textContent: plan.error }));
    footer.replaceChildren();
    const titleEditor = el("input", { value: plan.title, disabled: busy(), ariaLabel: "Tên kế hoạch",
      onchange: async event => { plan.title = event.target.value; await save(); } });
    footer.append(field("Tên kế hoạch", titleEditor));
    const invalid = !plan.tasks.length || hasCycle(plan.tasks) || plan.tasks.some(task => !task.title.trim() || !task.detail.trim()
      || (task.depends_on || []).some(id => !plan.tasks.some(t => t.id === id)));
    if (invalid) footer.append(el("p", { className: "dc-error", textContent: "Kiểm tra tên/chi tiết công việc và phụ thuộc (không được có chu trình)." }));
    footer.append(button("+ Thêm task", async () => {
      plan.tasks.push({ id: Math.max(0, ...plan.tasks.map(t => t.id)) + 1, title: "Công việc mới", detail: "Mô tả kết quả mong đợi",
        assignee: state.config.providers[0]?.id || "", model: "", effort: "default", kind: "code", depends_on: [], status: "chờ", log: [], summary: "" });
      await save(); render();
    }, busy()));
    if (!plan.approved) footer.append(button("Duyệt kế hoạch", async () => { plan.approved = true; await persistPlan(plan); render(); }, invalid || busy()));
    if (plan._running) footer.append(button("Dừng", stopWholePlan));
    else if (plan.approved && done < plan.tasks.length) {
      footer.append(button(plan.tasks.some(t => ["lỗi", "đã dừng", "cần sửa"].includes(t.status)) ? "Chạy tiếp" : "▶ Chạy tất cả",
        () => { confirm = true; render(); }, busy() || invalid));
    }
    if (confirm) footer.append(el("div", { className: "dc-confirm" }, [
      el("span", { textContent: `Chạy ${plan.tasks.length - done} công việc còn lại?` }),
      button("Chạy", async () => { confirm = false; await runWholePlan(plan); }, busy() || invalid),
      button("Để tôi xem trước", () => { confirm = false; render(); })]));
    footer.append(button("⇅ Xác minh kế hoạch", () => verifyPlan(plan), busy() || !state.mutis?.enabled || !state.mutis?.verify_gate),
      el("small", { textContent: `Tổng token: ${plan.tasks.reduce((n, task) => n + (task.tokens || 0), 0)}` }));
    footer.append(button("Lập lại kế hoạch", async () => {
      const { planFromRequest } = await import("./plans.js");
      await planFromRequest(plan.request, { appendRequest: false });
    }, busy()));
    const auto = el("input", { type: "checkbox", checked: !!plan.auto_continue,
      onchange: async event => { plan.auto_continue = event.target.checked; await persistPlan(plan); } });
    footer.append(field("Tự chạy tiếp", auto));
  }
  timer = setInterval(tick, 1000); tick();
  return { el: root, get plan() { return plan; }, log(text) { logs.textContent += `${text}\n`; },
    async refresh() {
      if (!plan || planning || busy() || fetching) return;
      fetching = true;
      try { this.load(await api(`/api/plans/${encodeURIComponent(plan.id)}`)); }
      catch (error) { showToast(error.message); }
      finally { fetching = false; }
    },
    load(value, live = false) {
      plan = value; liveStream = live; planning = live || !!value.planning;
      if (value.id) planCards.set(value.id, this);
      if (!planning) { clearInterval(timer); timer = null; }
      render();
    }, fail(text) { planning = false; clearInterval(timer); status.textContent = text; cancel.hidden = true; this.log(text); },
    update(value) { plan = value; render(); } };
}

export function storedPlanCard(id) {
  if (planCards.has(id)) return planCards.get(id);
  const card = createPlanCard();
  planCards.set(id, card);
  api(`/api/plans/${encodeURIComponent(id)}`).then(plan => card.load(plan)).catch(error => card.fail(error.message));
  return card;
}
window.addEventListener("plan-progress", event => {
  planCards.get(event.detail.id)?.update(event.detail);
  for (const message of state.messages) if (message.plan_id === event.detail.id) message.content = planSummary(event.detail);
});
window.addEventListener("debate-busy", () => {
  for (const card of planCards.values()) if (card.plan) card.update(card.plan);
});
