import { planRunOrder } from "./plan-order.js";
export { planRunOrder } from "./plan-order.js";
import { hideMemory, memoryEvent } from "./memory.js";
import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { EFFORTS, findProvider } from "./mode-chip.js";
import { devChatPreamble, folderInput } from "./chat.js";
import { api, readSse } from "./api.js";

export let activePlanRun = null;
function planProgress(plan) {
  if (activePlanRun?.plan === plan) activePlanRun.onProgress(plan);
  window.dispatchEvent(new CustomEvent("plan-progress", { detail: plan }));
}
export async function stopPlanTask(task, plan = state.currentPlan) {
  task._stopped = true;
  const run = activePlanRun?.plan === plan ? activePlanRun : null;
  if (run) { run.stopped = true; run.controller.abort(); }
  const chatId = run?.chatId || task.chat_id;
  if (chatId) await api(`/api/chat/${encodeURIComponent(chatId)}/stop`, { method: "POST" });
}
export async function stopWholePlan() {
  if (!activePlanRun) return;
  await stopPlanTask(activePlanRun.task || {}, activePlanRun.plan);
}
export async function runWholePlan(plan, onProgress = () => {}) {
  if (activePlanRun || state.planRunning || state.streaming) throw new Error("Đang có công việc chạy");
  if (!plan.approved) throw new Error("Kế hoạch chưa được duyệt");
  planRunOrder(plan.tasks);
  activePlanRun = { plan, onProgress, stopped: false, task: null, controller: new AbortController() };
  plan._running = true;
  const run = activePlanRun;
  window.dispatchEvent(new Event("debate-busy"));
  try {
    while (!run.stopped) {
      const id = planRunOrder(plan.tasks)[0];
      if (id === undefined) break;
      const task = plan.tasks.find(item => item.id === id);
      run.task = task;
      try { await runPlanTask(task, plan); }
      catch (error) {
        task.status = run.stopped ? "đã dừng" : "lỗi";
        task.log ||= [];
        task.log.push(error.message);
        await persistPlan(plan);
      }
      if (run.stopped) { task.status = "đã dừng"; await persistPlan(plan); }
      planProgress(plan);
      if (task.status !== "xong") break;
    }
  } finally {
    try {
      if (run.stopped && run.task) {
        run.task.status = "đã dừng";
        await persistPlan(plan);
      }
    } finally {
      activePlanRun = null;
      plan._running = false;
      planProgress(plan);
      onProgress(plan);
      window.dispatchEvent(new Event("debate-busy"));
      if (state.currentPlan?.id === plan.id) renderPlanEditor();
    }
  }
  return plan;
}

export function renderPlanList() {
  const root = $("#plans-list");
  root.replaceChildren();
  if (!state.plans.length) { root.textContent = "Chưa có kế hoạch."; return; }
  for (const plan of state.plans) {
    root.append(el("button", { className: "plan-list-item",
      textContent: `${plan.title} · ${plan.tasks.length} công việc${plan.approved ? " · đã duyệt" : " · bản nháp"}`,
      onclick: () => openPlan(plan) }));
  }
}
export async function planFromRequest(text, { stay = false, container = null } = {}) {
  const request = text.trim() || [...state.messages].reverse().find(message => message.role === "user")?.content || "";
  if (!request) { showToast("Nhập yêu cầu hoặc mở chat có yêu cầu trước."); return; }
  const buttons = [$("#plan-button"), $("#hint-plan")];
  buttons.forEach(button => { if (button) button.disabled = true; });
  if (!stay) {
    $(".main-panel").hidden = true;
    $("#self-dev-view").hidden = true;
    $("#plans-view").hidden = false;
  }
  const provider = state.config.providers.find(item => item.id === $("#planner-provider").value);
  const root = container || $("#plans-list");
  root.replaceChildren();
  const card = el("section", { className: "plan-progress" });
  const label = el("p", { textContent: `Đang lập kế hoạch bằng ${provider?.name || "provider"} · 00:00` });
  const spinner = el("span", { className: "spinner", ariaLabel: "Đang lập kế hoạch" });
  const logs = el("pre", { className: "task-log" });
  const cancel = el("button", { className: "subtle-button", textContent: "Hủy" });
  const controller = new AbortController();
  cancel.onclick = () => controller.abort();
  card.append(label, spinner, logs, cancel);
  root.append(card);
  const started = Date.now();
  const timer = setInterval(() => {
    const seconds = Math.floor((Date.now() - started) / 1000);
    label.textContent = `Đang lập kế hoạch bằng ${provider?.name || "provider"} · ${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  }, 1000);
  try {
    if (state.mutis?.enabled) await api("/api/mutis/roles/resolve");
    const response = await fetch("/api/plans", { method: "POST", signal: controller.signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        request,
        folder: folderInput.value.trim(),
        dev_chat: state.devChat,
        provider_id: state.mutis?.roles?.lead?.provider_id
          || $("#planner-provider").value
          || state.config.planner_provider_id,
        model: state.mutis?.roles?.lead?.model
          || $("#planner-model").value
          || state.config.planner_model,
      }),
    });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    if (!response.body) throw new Error("Browser does not support response streaming");
    let receivedPlan = null;
    await readSse(response.body, (event, data) => {
      if (event === "log") logs.textContent += `${data.text || ""}\n`;
      if (event === "error") throw new Error(data.message || data.text || "Planning failed");
      if (event === "plan") receivedPlan = data;
    });
    if (receivedPlan) { await loadPlans({ preserveDebate: stay }); if (!stay) openPlan(receivedPlan); return receivedPlan; }
  } catch (error) {
    if (error.name === "AbortError") {
      logs.textContent += "\u0110\u00e3 hu\u1ef7\n";
      label.textContent = "\u0110\u00e3 hu\u1ef7";
    }
    else { logs.textContent += `${error.message}\n`; }
  } finally { clearInterval(timer); buttons.forEach(button => { if (button) button.disabled = false; }); }
}
export function openPlan(plan) {
  state.currentPlan = activePlanRun?.plan.id === plan.id ? activePlanRun.plan : structuredClone(plan);
  $(".main-panel").hidden = true;
  $("#self-dev-view").hidden = true;
  $("#plans-view").hidden = false;
  renderPlanEditor();
}
export function renderPlanEditor() {
  const root = $("#plan-editor"), plan = state.currentPlan;
  if (!plan) return;
  root.hidden = false;
  root.replaceChildren();
  const title = el("input", { className: "plan-title", value: plan.title, oninput: event => plan.title = event.target.value });
  root.append(title);
  const table = el("table", { className: "plan-table" });
  table.innerHTML = "<thead><tr><th>#</th><th>Công việc</th><th>Giao cho</th><th>Model / effort</th><th>Phụ thuộc</th><th>Loại</th><th>Trạng thái</th><th></th></tr></thead>";
  const body = el("tbody");
  for (const [index, task] of plan.tasks.entries()) {
    const row = el("tr"), cell = child => row.append(el("td", {}, [child]));
    cell(el("span", { textContent: task.id }));
    const editor = el("div");
    editor.append(
      el("input", {
        value: task.title,
        placeholder: "Tên việc",
        oninput: event => task.title = event.target.value,
      }),
      el("textarea", {
        value: task.detail,
        placeholder: "Tệp cần sửa và kết quả mong đợi",
        oninput: event => task.detail = event.target.value,
      }),
    );
    cell(editor);
    const assignee = el("select");
    for (const provider of state.config.providers) assignee.append(el("option", { value: provider.id, textContent: provider.name }));
    assignee.value = findProvider(task.assignee)?.id ?? task.assignee;
    assignee.onchange = () => {
      task.assignee = assignee.value;
      task.model = state.config.providers.find(p => p.id === task.assignee)?.models[0] || "";
      renderPlanEditor();
    };
    const assigneeCell = el("div");
    assigneeCell.append(assignee);
    if (task.reassigned_from) assigneeCell.append(el("small", {
      textContent: `⚠ đã đổi từ ${task.reassigned_from} (chưa kết nối)`,
    }));
    cell(assigneeCell);
    const provider = findProvider(task.assignee);
    const settings = el("div"), model = el("select");
    for (const item of provider?.models || []) model.append(el("option", { value: item, textContent: item }));
    model.value = task.model || provider?.models[0] || ""; model.onchange = () => task.model = model.value;
    const effort = el("select");
    for (const item of EFFORTS) effort.append(el("option", { value: item, textContent: item }));
    effort.value = task.effort || "default"; effort.onchange = () => task.effort = effort.value;
    settings.append(model, effort); cell(settings);
    const deps = el("input", { value: (task.depends_on || []).join(",") });
    const depError = el("small", { className: "dependency-error" });
    const validateDeps = () => {
      task.depends_on = deps.value.split(",").map(value => value.trim()).filter(Boolean).map(Number);
      const invalid = task.depends_on.some(id => !plan.tasks.some(item => item.id === id));
      depError.textContent = invalid ? "Mã phụ thuộc không tồn tại." : "";
      approve.disabled = invalid || hasCycle(plan.tasks);
    };
    deps.oninput = validateDeps; const depwrap = el("div"); depwrap.append(deps, depError); cell(depwrap);
    const kind = el("select");
    for (const value of ["plan", "small_edit", "code", "test", "docs", "review"]) {
      kind.append(el("option", { value, textContent: value }));
    }
    kind.value = task.kind;
    kind.onchange = () => task.kind = kind.value;
    cell(kind);
    const status = el("div", { dataset: { taskStatus: String(task.id) } });
    status.append(el("span", { textContent: `${task.status === "waiting" ? "chờ" : task.status}${task.tokens ? ` · ${task.tokens} tokens` : ""}${task.log?.length ? ` · ${task.log.at(-1)}` : ""}` }));
    if (task.verify) status.append(el("span", { className: "gate-chip", textContent: `Kiểm tra: ${task.verify.ok ? "ĐẠT" : "LỖI"}` }));
    if (task.review) {
      status.append(el("span", {
        className: "gate-chip",
        textContent: task.review.verdict === "SKIPPED"
          ? "Review: bỏ qua"
          : `Review: ${task.review.verdict || "FAIL"} (${task.review.issues?.length || 0} vấn đề)`,
      }));
    }
    if (task.verify) status.append(el("details", {}, [el("summary", { textContent: "Tóm tắt kiểm tra" }), el("pre", { textContent: (task.verify.summary_lines || []).join("\n") })]));
    if (task.review) status.append(el("details", {}, [el("summary", { textContent: "Vấn đề review" }), el("pre", { textContent: JSON.stringify(task.review.issues || [], null, 2) })]));
    cell(status);
    const actions = el("div");
    const moveUp = () => {
      [plan.tasks[index - 1], plan.tasks[index]] = [plan.tasks[index], plan.tasks[index - 1]];
      renderPlanEditor();
    };
    const moveDown = () => {
      [plan.tasks[index + 1], plan.tasks[index]] = [plan.tasks[index], plan.tasks[index + 1]];
      renderPlanEditor();
    };
    const removeTask = () => {
      plan.tasks.splice(index, 1);
      renderPlanEditor();
    };
    actions.append(
      el("button", { textContent: "↑", title: "Lên", disabled: index === 0, onclick: moveUp }),
      el("button", { textContent: "↓", title: "Xuống", disabled: index === plan.tasks.length - 1, onclick: moveDown }),
      el("button", { textContent: "×", title: "Xoá", onclick: removeTask }),
    );
    if (plan.approved) {
      const isRunning = task.status === "đang chạy";
      const depsDone = task.depends_on.every(id => plan.tasks.find(item => item.id === id)?.status === "xong");
      const finished = ["xong", "lỗi", "đã dừng"].includes(task.status);
      actions.append(el("button", {
        textContent: finished ? "Chạy lại" : "Chạy",
        title: depsDone ? "" : "Chờ các công việc phụ thuộc xong",
        disabled: isRunning || !!activePlanRun || !depsDone,
        onclick: () => runPlanTask(task),
      }));
      if (isRunning) {
        actions.append(el("button", {
          textContent: "Dừng",
          onclick: async () => {
            await stopPlanTask(task, plan);
          },
        }));
      }
      if (task.status !== "xong" && !isRunning) {
        actions.append(el("button", {
          textContent: "Đánh dấu xong",
          onclick: async () => { task.status = "xong"; await persistPlan(plan); renderPlanEditor(); },
        }));
      }
      actions.append(el("button", { textContent: "Review lại", disabled: isRunning, onclick: () => runTaskGates(task) }));
    }
    cell(actions); body.append(row);
  }
  table.append(body); root.append(table);
  const approve = el("button", {
    textContent: plan.approved ? "Đã duyệt" : "Duyệt kế hoạch",
    disabled: plan.approved || hasCycle(plan.tasks),
    onclick: async () => {
      plan.approved = true;
      await persistPlan(plan);
      renderPlanEditor();
    },
  });
  const add = el("button", {
    textContent: "+ Thêm công việc",
    onclick: () => {
      plan.tasks.push({
        id: Math.max(0, ...plan.tasks.map(task => task.id)) + 1,
        title: "Công việc mới", detail: "", assignee: state.config.providers[0]?.id || "",
        model: "", effort: "default", depends_on: [], kind: "code", status: "chờ",
        summary: "", log: [], tokens: 0,
      });
      renderPlanEditor();
    },
  });
  const regen = el("button", { textContent: "Lập lại kế hoạch", onclick: () => planFromRequest(plan.request) });
  const gates = el("div", { className: "plan-gates" });
  gates.append(el("button", { textContent: "Chạy kiểm tra", disabled: !state.mutis?.enabled || !state.mutis?.verify_gate, onclick: () => verifyPlan(plan) }));
  if (state.mutis?.enabled) gates.append(el("label", {}, [
    el("input", {
      type: "checkbox",
      checked: !!plan.auto_continue,
      onchange: event => {
        plan.auto_continue = event.target.checked;
        persistPlan(plan);
      },
    }),
    " Tự chạy tiếp",
  ]));
  root.append(el("p", { className: "token-usage", textContent: `Tổng token kế hoạch: ${plan.tasks.reduce((sum, task) => sum + (task.tokens || 0), 0)}` }), gates, add, regen, approve);
  for (const task of plan.tasks) if (task.log?.length) root.append(el("pre", { className: "task-log", textContent: task.log.join("\n") }));
  if (activePlanRun) {
    for (const control of root.querySelectorAll("input, select, textarea, button")) {
      control.disabled = control.textContent !== "Dừng";
    }
    if (activePlanRun.plan === plan) root.append(el("button", { textContent: "Dừng kế hoạch", onclick: stopWholePlan }));
  }
}
export function hasCycle(tasks) {
  const byId = new Map(tasks.map(task => [task.id, task]));
  const visiting = new Set(), done = new Set();
  const visit = id => {
    if (done.has(id)) return false;
    if (visiting.has(id)) return true;
    visiting.add(id);
    for (const dep of byId.get(id)?.depends_on || []) {
      if (byId.has(dep) && visit(dep)) return true;
    }
    visiting.delete(id);
    done.add(id);
    return false;
  };
  return tasks.some(task => visit(task.id));
}
export async function persistPlan(plan = state.currentPlan) {
  // Keep the local plan object as the source of truth: running tasks hold references into it,
  // so replacing it with the server copy would make their later status updates get lost.
  const body = JSON.stringify(plan, (key, value) => (key.startsWith("_") ? undefined : value));
  await api(`/api/plans/${plan.id}`, { method: "PUT", body });
  planProgress(plan);
  await loadPlans({ preserveDebate: true });
}
// Task kinds that may change files (small_edit used to be treated as read-only by mistake).
function taskCanWrite(task) {
  return ["code", "test", "docs", "small_edit"].includes(task.kind);
}

export async function runPlanTask(task, plan = state.currentPlan) {
  state.planRunning = (state.planRunning || 0) + 1;
  window.dispatchEvent(new Event("debate-busy"));
  try { return await runPlanTaskImpl(task, plan); }
  finally {
    state.planRunning -= 1;
    window.dispatchEvent(new Event("debate-busy"));
  }
}
async function runPlanTaskImpl(task, plan) {
  if (activePlanRun?.stopped) { task.status = "đã dừng"; return; }
  task._stopped = false; task.summary = ""; task.diff = "";
  const provider = findProvider(task.assignee);
  if (!provider) throw new Error(`Không tìm thấy nguồn ${task.assignee}`);
  if (state.mutis?.enabled) await api("/api/mutis/roles/resolve");
  const prompt = `${plan.dev_chat ? devChatPreamble(plan.folder) + "\n\n" : ""}${state.mutis?.enabled ? `${state.mutis.worker_rules || ""}\n\n` : ""}Kế hoạch: ${plan.title}
Công việc: ${task.title}
Chi tiết: ${task.detail}
Tóm tắt công việc phụ thuộc đã hoàn tất:
${task.depends_on.map(id => plan.tasks.find(item => item.id === id)?.summary || "").join("\n")}`;
  if (activePlanRun?.plan === plan) activePlanRun.task = task;
  const chatId = task.chat_id || crypto.randomUUID(); task.chat_id = chatId; task.status = "đang chạy"; task.log = [];
  planProgress(plan);
  await persistPlan(plan); renderPlanEditor();
  let hadError = false;
  try {
    const response = await api("/api/chat", {
      signal: activePlanRun?.plan === plan ? activePlanRun.controller.signal : undefined,
      method: "POST",
      body: JSON.stringify({
        chat_id: chatId, dev_chat: Boolean(plan.dev_chat), plan_id: plan.id, task_id: String(task.id), task_kind: task.kind, provider_id: provider.id,
        model: task.model || provider.models[0] || "", effort: task.effort || "default",
        folder: plan.folder,
        permission: taskCanWrite(task) ? "write" : "read",
        // Sources with agent mode read/edit files through file tools; other sources ignore this field.
        file_tools: provider.agent_capable ? (taskCanWrite(task) ? "write" : "read") : "none",
        messages: [{ role: "user", content: prompt }],
      }),
    });
    await readSse(response.body, (event, data) => {
      if (event === "memory") memoryEvent(data, plan.folder);
      if (event === "error") { hadError = true; task.log.push(`Lỗi: ${data.text || "Không có chi tiết"}`); }
      if (event === "delta") task.summary += data.text || "";
      if (event === "diff") task.diff += data.text || data.diff || "";
      if (event === "log") {
        const text = data.text || "";
        task.log.push(text);
        const usage = text.match(/^Token: vào (\d+).*?ra (\d+)/);
        if (usage) task.tokens = (task.tokens || 0) + Number(usage[1]) + Number(usage[2]);
      }
      planProgress(plan);
      const node = $(`[data-task-status="${task.id}"]`);
      if (node) node.textContent = `${task.status} · ${task.log.at(-1) || ""}`;
    });
    task.status = task._stopped ? "đã dừng" : hadError ? "lỗi" : "xong";
  } catch (error) { hadError = true; task.status = task._stopped ? "đã dừng" : "lỗi"; task.log.push(error.message); }
  task.summary = hideMemory(task.summary);
  await persistPlan(plan);
  if (!hadError && !task._stopped) await runTaskGates(task, plan);
  renderPlanEditor();
  if (!activePlanRun && plan.auto_continue && task.status === "xong") {
    const next = plan.tasks.find(item => item.status !== "xong" && item.status !== "lỗi" && item.depends_on.every(id => plan.tasks.find(dep => dep.id === id)?.status === "xong"));
    if (next) await runPlanTask(next, plan);
  }
}
export async function verifyPlan(plan) {
  const response = await api("/api/verify", { method: "POST", body: JSON.stringify({ folder: plan.folder }) });
  await readSse(response.body, (event, data) => { if (event === "log") showToast(data.text || ""); if (event === "verify") showToast((data.summary_lines || []).join(" · ")); });
}
export function extractJson(text) {
  let start = -1, depth = 0, quoted = false, escape = false;
  for (let i = 0; i < text.length; i++) { const c = text[i]; if (start < 0) { if (c === "{") { start = i; depth = 1; } continue; }
    if (quoted) { if (escape) escape = false; else if (c === "\\") escape = true; else if (c === '"') quoted = false; continue; }
    if (c === '"') quoted = true; else if (c === "{") depth++; else if (c === "}" && --depth === 0) { try { return JSON.parse(text.slice(start, i + 1)); } catch (_) { start = -1; } }
  } return null;
}

// Build a fix task that keeps the ORIGINAL task context (title + detail) so the worker knows what the task was,
// instead of nesting "Sửa lỗi …: Sửa lỗi …: X" and losing the original request.
export function buildFixTask(plan, task, label, issuesText) {
  const rootTitle = task.fix_root_title || task.title;
  const rootDetail = task.fix_root_detail ?? task.detail;
  const round = (task.fix_rounds_used || 0);
  return {
    ...structuredClone(task),
    id: Math.max(0, ...plan.tasks.map(t => t.id)) + 1,
    title: `${label}: ${rootTitle} (vòng ${round})`,
    detail: `Nhiệm vụ gốc: ${rootTitle}\n${rootDetail}\n\nLỗi cần sửa (${label.toLowerCase()}):\n${issuesText}`,
    fix_root_title: rootTitle,
    fix_root_detail: rootDetail,
    depends_on: [task.id],
    status: "chờ",
    summary: "",
    log: [],
    tokens: 0,
    chat_id: undefined,
    verify: null,
    review: null,
  };
}
export async function runTaskGates(task, plan = state.currentPlan) {
  const cfg = state.mutis;
  if (!cfg?.enabled || activePlanRun?.stopped) return;
  if (cfg.verify_gate) {
    const response = await api("/api/verify", {
      method: "POST", body: JSON.stringify({ folder: plan.folder }),
      signal: activePlanRun?.plan === plan ? activePlanRun.controller.signal : undefined,
    });
    await readSse(response.body, (event, data) => { if (event === "verify") task.verify = data; if (event === "log") task.log.push(data.text || ""); });
    // Record the verify outcome so MutiS stats and self-diagnosis see it (found by the self-diagnosis: verify_fail_rate stayed 0).
    await api("/api/mutis/events", {
      method: "POST",
      body: JSON.stringify({
        plan_id: plan.id,
        task_id: String(task.id),
        task_kind: task.kind,
        provider_id: task.assignee,
        kind: "verify",
        ok: !!task.verify?.ok,
        detail: (task.verify?.summary_lines || []).join(" | ").slice(0, 2000),
      }),
    }).catch(() => {});
    if (activePlanRun?.stopped) return;
    if (!task.verify?.ok) {
      const action = nextReviewAction(task, {
        verdict: "FAIL",
        issues: (task.verify?.summary_lines || []).map(problem => ({ severity: "high", problem })),
      }, task.fix_rounds_used ?? 0, cfg.fix_rounds_max);
      if (cfg.auto_fix && action === "create_fix") {
        task.status = "cần sửa"; task.fix_rounds_used = (task.fix_rounds_used || 0) + 1;
        const fix = buildFixTask(plan, task, "Sửa lỗi kiểm tra", (task.verify.summary_lines || []).join("\n"));
        plan.tasks.push(fix); await persistPlan(plan); await runPlanTask(fix, plan);
        await api("/api/mutis/events", {
          method: "POST",
          body: JSON.stringify({
            plan_id: plan.id,
            task_id: String(fix.id),
            provider_id: fix.assignee,
            kind: "fix_task",
            ok: fix.status === "xong",
            detail: `Task sửa verify được tạo cho ${task.id}`,
          }),
        });
        task.status = fix.status === "xong" ? "xong" : "lỗi";
      } else task.status = "lỗi";
      await persistPlan(plan); return;
    }
  }
  if (cfg.auto_review && ["code", "test", "docs", "small_edit"].includes(task.kind)) {
    const resolved = await api("/api/mutis/roles/resolve");
    const resolvedLead = resolved.lead || {};
    const lead = cfg.roles?.lead;
    const reviewer = resolvedLead.used_provider;
    if (!reviewer) {
      task.review = { verdict: "SKIPPED", issues: [], notes: "Không có nguồn review đang kết nối" };
      task.status = task.verify?.ok === false ? "lỗi" : "xong";
      await api("/api/mutis/events", {
        method: "POST",
        body: JSON.stringify({
          plan_id: plan.id,
          task_id: String(task.id),
          provider_id: "",
          kind: "review",
          ok: true,
          detail: JSON.stringify(task.review),
        }),
      });
      await persistPlan(plan);
      return;
    }
    const gitDiff = task.diff ? "" : (await api("/api/plan-diff", { method: "POST", body: JSON.stringify({ folder: plan.folder }) })).diff;
    const prompt = [
      `Review task và trả CHỈ JSON theo schema {"verdict":"PASS|FAIL","issues":[{"severity":"high|medium|low","file":"","line":0,"problem":"","fix":""}],"notes":""}. Task: ${task.title}\n`,
      `Chi tiết: ${task.detail}\nTóm tắt: ${task.summary}\n`,
      `Verify: ${(task.verify?.summary_lines || []).join("\n")}\nDiff (nếu có):\n`,
      (task.diff || gitDiff || "").slice(0, 30000),
    ].join("");
    if (activePlanRun?.stopped) return;
    const reviewModel = resolvedLead.used_provider.id === lead?.provider_id ? (lead?.model || reviewer.models[0] || "") : (reviewer.models[0] || "");
    let answer = ""; const chatId = crypto.randomUUID();
    if (activePlanRun?.plan === plan) activePlanRun.chatId = chatId;
    const response = await api("/api/chat", {
      signal: activePlanRun?.plan === plan ? activePlanRun.controller.signal : undefined,
      method: "POST",
      body: JSON.stringify({
        chat_id: chatId,
        plan_id: plan.id,
        task_id: String(task.id),
        task_kind: task.kind,
        provider_id: reviewer.id,
        model: reviewModel,
        effort: lead?.effort || "default",
        folder: plan.folder,
        permission: "read",
        messages: [{ role: "user", content: prompt }],
      }),
    });
    await readSse(response.body, (event, data) => { if (event === "delta") answer += data.text || ""; });
    if (activePlanRun?.plan === plan) activePlanRun.chatId = null;
    if (activePlanRun?.stopped) { task.status = "đã dừng"; await persistPlan(plan); return; }
    task.review = extractJson(answer) || {
      verdict: "FAIL",
      issues: [{
        severity: "high",
        file: "",
        line: 0,
        problem: "Reviewer trả JSON không hợp lệ",
        fix: "Review lại",
      }],
      notes: answer.slice(-1000),
    };
    await api("/api/mutis/events", {
      method: "POST",
      body: JSON.stringify({
        plan_id: plan.id,
        task_id: String(task.id),
        provider_id: reviewer.id,
        kind: "review",
        ok: task.review.verdict === "PASS",
        detail: JSON.stringify(task.review).slice(0, 4000),
      }),
    });
    if (task.review.verdict === "FAIL") {
      if (cfg.auto_fix && nextReviewAction(task, task.review, task.fix_rounds_used ?? 0, cfg.fix_rounds_max) === "create_fix") {
        task.status = "cần sửa"; task.fix_rounds_used++;
        const fix = buildFixTask(plan, task, "Sửa lỗi review", (task.review.issues || []).map(i => `${i.severity} ${i.file}:${i.line} ${i.problem} — ${i.fix}`).join("\n"));
        plan.tasks.push(fix); await persistPlan(plan); await runPlanTask(fix, plan);
        await api("/api/mutis/events", {
          method: "POST",
          body: JSON.stringify({
            plan_id: plan.id,
            task_id: String(fix.id),
            provider_id: reviewer.id,
            kind: "fix_task",
            ok: fix.status === "xong",
            detail: `Task sửa được tạo cho ${task.id}`,
          }),
        });
        task.status = fix.status === "xong" ? "xong" : "lỗi";
      } else { task.status = "lỗi"; showToast("Review thất bại; đã hết vòng sửa tự động"); }
    }
  }
  await persistPlan(plan);
}
export function nextReviewAction(task, review, roundsUsed, maxRounds) {
  if (review?.verdict === "SKIPPED") return "skip";
  if (review?.verdict !== "FAIL") return "complete";
  const hasBlockingIssue = (review.issues || []).some(issue => ["high", "medium"].includes(issue.severity));
  return hasBlockingIssue && roundsUsed < maxRounds ? "create_fix" : "fail";
}

export async function loadPlans({ preserveDebate = false } = {}) {
  state.plans = await api("/api/plans");
  if (!preserveDebate || !$("#plans-list").querySelector(".debate-card")) renderPlanList();
}
