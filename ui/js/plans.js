import { taskCanWrite } from "./plan-helpers.js";
export { hasCycle } from "./plan-helpers.js";
import { planRunOrder } from "./plan-order.js";
export { planRunOrder } from "./plan-order.js";
import { hideMemory, memoryEvent } from "./memory.js";
import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { findProvider } from "./mode-chip.js";
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
    }
  }
  return plan;
}

export async function planFromRequest(text = "", { appendRequest = true, afterDebateId = null } = {}) {
  appendRequest = appendRequest && Boolean(text.trim());
  const request = text.trim() || [...state.messages].reverse().find(message => message.role === "user")?.content || "";
  if (!request || state.streaming || state.planRunning || state.activeDebates?.size) {
    showToast("Nhập yêu cầu và chờ công việc hiện tại hoàn tất."); return;
  }
  const chat = await import("./chat.js");
  const { createPlanCard, planSummary } = await import("./plan-card.js");
  const folder = folderInput.value.trim();
  if (!folder) { showToast("Chọn thư mục làm việc trước."); return; }
  if (!state.chatId) { chat.newChat(); folderInput.value = folder; }
  chat.showChatPanel();
  const chatId = state.chatId;
  const memory = state.memoryChats.get(chatId) || { id: chatId };
  if (!memory.title || memory.title === "Chat mới") memory.title = request.slice(0, 72);
  memory.updated_at = Date.now() / 1000;
  state.memoryChats.set(chatId, memory);
  const card = createPlanCard();
  const assistant = { role: "assistant", content: "", _planCard: card };
  if (appendRequest) state.messages.push({ role: "user", content: request });
  const debateIndex = afterDebateId ? state.messages.findIndex(m => m.debate_id === afterDebateId) : -1;
  if (debateIndex >= 0) state.messages.splice(debateIndex + 1, 0, assistant);
  else state.messages.push(assistant);
  chat.messageInput.value = "";
  chat.renderMessages();
  const controller = new AbortController();
  card.el.addEventListener("plan-cancel", () => controller.abort());
  state.streaming = true;
  window.dispatchEvent(new Event("debate-busy"));
  let receivedPlan = null;
  try {
    if (state.mutis?.enabled) await api("/api/mutis/roles/resolve");
    const response = await fetch("/api/plans", { method: "POST", signal: controller.signal,
      headers: { "Content-Type": "application/json" }, body: JSON.stringify({ request, folder,
        chat_id: chatId, append_request: appendRequest, after_debate_id: afterDebateId, dev_chat: state.devChat,
        provider_id: state.mutis?.roles?.lead?.provider_id || $("#planner-provider").value || state.config.planner_provider_id,
        model: state.mutis?.roles?.lead?.model || $("#planner-model").value || state.config.planner_model,
      }) });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    await readSse(response.body, (event, data) => {
      if (event === "log") card.log(data.text || "");
      if (event === "error") throw new Error(data.message || data.text || "Lập kế hoạch thất bại");
      if (event === "planning") { assistant.plan_id = data.id; card.load(data, true); }
      if (event === "plan") { receivedPlan = data; assistant.plan_id = data.id; assistant.content = planSummary(data); card.load(data); }
    });
    if (!receivedPlan) throw new Error("Mất kết nối khi lập kế hoạch");
    return receivedPlan;
  } catch (error) {
    assistant.content = error.name === "AbortError" ? "Đã hủy lập kế hoạch" : error.message;
    card.fail(assistant.content);
  } finally {
    state.streaming = false;
    window.dispatchEvent(new Event("debate-busy"));
    chat.rememberCurrentChat();
    await chat.refreshChats();
    await loadPlans();
  }
}
export async function openPlan(plan) {
  const chat = await import("./chat.js");
  if (state.streaming) return;
  if (plan.chat_id) await chat.loadChat(plan.chat_id);
  else {
    chat.newChat();
    chat.folderInput.value = plan.folder;
    plan.chat_id = state.chatId;
    await persistPlan(plan);
    await chat.refreshChats();
    await chat.loadChat(plan.chat_id);
  }
  state.currentPlan = plan;
  chat.showChatPanel();
  await loadPlans();
}
export async function persistPlan(plan = state.currentPlan) {
  // Keep the local plan object as the source of truth: running tasks hold references into it,
  // so replacing it with the server copy would make their later status updates get lost.
  const body = JSON.stringify(plan, (key, value) => (key.startsWith("_") ? undefined : value));
  await api(`/api/plans/${plan.id}`, { method: "PUT", body });
  planProgress(plan);
  state.plans = await api("/api/plans");
}
export async function runPlanTask(task, plan = state.currentPlan) {
  if (!plan?.approved) throw new Error("Kế hoạch chưa được duyệt");
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
  await persistPlan(plan);
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

    });
    task.status = task._stopped ? "đã dừng" : hadError ? "lỗi" : "xong";
  } catch (error) { hadError = true; task.status = task._stopped ? "đã dừng" : "lỗi"; task.log.push(error.message); }
  task.summary = hideMemory(task.summary);
  await persistPlan(plan);
  if (!hadError && !task._stopped) await runTaskGates(task, plan);
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

export async function loadPlans() {
  state.plans = await api("/api/plans");
  const root = $("#legacy-plans");
  if (!root) return;
  root.replaceChildren();
  for (const plan of state.plans.filter(item => !item.chat_id)) {
    root.append(el("button", { className: "plan-list-item", textContent: `✦ ${plan.title}`, onclick: () => openPlan(plan) }));
  }
  const { renderDebateList } = await import("./debate.js");
  await renderDebateList();
}
