import { $, el, showToast } from "./util.js";
import { api } from "./api.js";
import { state } from "./state.js";
import { EFFORTS, EFFORT_KINDS, EFFORT_LABELS, setupProviderControls, renderModePicker, saveLastMode } from "./mode-chip.js";
import { activePlanRun, planFromRequest, openPlan, persistPlan, runWholePlan, stopWholePlan } from "./plans.js";

// This panel survives verdict re-renders, so confirmations and progress stay in the card.
export function createDebateActions(getRecord, onProgress) {
  const root = el("section", { className: "dc-plan" });
  const controls = el("div", { className: "dc-actions" });
  const body = el("div");
  const chooser = el("div", { className: "dc-execute" });
  let plan = null;
  let loadedId = null;
  let busy = false;
  let confirm = false;
  let errorText = "";
  let refreshing = false;
  const blocked = () => busy || state.streaming || state.planRunning || state.activeDebates?.size || !!activePlanRun || getRecord().status !== "done";
  const split = el("button", { textContent: "✦ Chia việc & thực hiện", onclick: makePlan });
  const direct = el("button", { textContent: "▶ Thực hiện với…", onclick: chooseSource });
  controls.append(split, direct);
  root.append(controls, body, chooser);

  function buttons() {
    split.disabled = direct.disabled = blocked();
    for (const button of chooser.querySelectorAll("button")) button.disabled = blocked() || !chooser.querySelector("select")?.value;
  }
  function updateProgress() {
    onProgress(plan);
    renderPlan();
    buttons();
  }
  async function refreshPlan() {
    const id = getRecord().plan_id;
    if (!id || refreshing || busy || activePlanRun?.plan.id === id) return;
    refreshing = true;
    try {
      const saved = await api(`/api/plans/${encodeURIComponent(id)}`);
      if (busy || activePlanRun?.plan.id === id || getRecord().plan_id !== id) return;
      plan = saved;
      errorText = "";
      loadedId = id;
      updateProgress();
    } catch (error) { errorText = error.message; renderPlan(); }
    finally { refreshing = false; }
  }
  function renderPlan() {
    body.replaceChildren();
    if (errorText) body.append(el("p", { className: "dc-error", textContent: errorText }));
    if (!plan) return;
    body.append(el("strong", { textContent: plan.title }));
    const list = el("ul", { className: "dc-task-list" });
    const symbols = { "xong": "✓", "đang chạy": "◐", "lỗi": "✗", "cần sửa": "◐" };
    for (const task of plan.tasks) {
      const name = state.config.providers.find(provider => provider.id === task.assignee)?.name || task.assignee;
      list.append(el("li", { textContent: `${symbols[task.status] || "○"} ${task.title} · ${name} · ${task.kind} · ${task.status}` }));
      if (["lỗi", "cần sửa"].includes(task.status)) {
        const gateError = task.verify?.ok === false ? task.verify.summary_lines?.join(" · ")
          : task.review?.verdict === "FAIL" ? task.review.issues?.map(issue => issue.problem).join(" · ") || task.review.notes : "";
        const detail = gateError || task.log?.findLast(line => line.startsWith("Lỗi:")) || task.log?.at(-1) || "Công việc chưa đạt kiểm tra/review";
        list.append(el("li", { className: "dc-error", textContent: detail }));
      }
    }
    body.append(list, el("a", { href: "#", textContent: "Mở kế hoạch", onclick: event => {
      event.preventDefault(); openPlan(activePlanRun?.plan.id === plan.id ? activePlanRun.plan : plan);
    } }));
    if (getRecord().plan_id !== plan.id) return;
    if (activePlanRun?.plan.id === plan.id) {
      body.append(el("button", { textContent: "Dừng", onclick: () => stopWholePlan().catch(error => showToast(error.message)) }));
    } else if (confirm) {
      body.append(el("div", { className: "dc-confirm" }, [
        el("span", { textContent: `Duyệt và chạy ${plan.tasks.length} công việc?` }),
        el("button", { textContent: "Chạy", disabled: blocked(), onclick: executePlan }),
        el("button", { textContent: "Để tôi xem trước", onclick: () => { confirm = false; renderPlan(); openPlan(plan); } }),
      ]));
    } else if (plan.tasks.some(task => task.status !== "xong")) {
      body.append(el("button", { textContent: plan.approved ? "Chạy tiếp" : "Duyệt và chạy", disabled: blocked(),
        onclick: () => { if (plan.approved) executePlan(); else { confirm = true; renderPlan(); } } }));
    }
  }
  async function makePlan() {
    if (blocked()) return;
    busy = true; buttons(); errorText = "";
    try {
      const debate = await api(`/api/debates/${encodeURIComponent(getRecord().id)}`);
      if (debate.plan_id) {
        getRecord().plan_id = debate.plan_id;
        plan = await api(`/api/plans/${encodeURIComponent(debate.plan_id)}`);
      } else {
        const { folderInput } = await import("./chat.js");
        folderInput.value = debate.folder;
        plan ||= await planFromRequest(debate.plan_request || "", { stay: true, container: body });
        if (!plan) throw new Error("Không tạo được kế hoạch; hãy thử lại");
        // Keep the generated plan available if saving the link fails; retry never creates another plan.
        await api(`/api/debates/${encodeURIComponent(debate.id)}/plan`, {
          method: "POST", body: JSON.stringify({ plan_id: plan.id }),
        });
        getRecord().plan_id = plan.id;
      }
      loadedId = plan.id; confirm = !plan.approved;
    } catch (error) { errorText = error.message; }
    finally { busy = false; updateProgress(); }
  }
  async function executePlan() {
    if (blocked() || !plan) return;
    busy = true; confirm = false; errorText = ""; buttons();
    try {
      plan.approved = true;
      await persistPlan(plan);
      // runWholePlan publishes progress without opening the editor or changing chat view.
      busy = false;
      await runWholePlan(plan, current => { plan = current; updateProgress(); });
    } catch (error) { errorText = error.message; }
    finally { busy = false; updateProgress(); }
  }
  async function chooseSource() {
    if (blocked()) return;
    if (chooser.childElementCount) { chooser.replaceChildren(); return; }
    busy = true; buttons();
    chooser.textContent = "Đang kiểm tra kết nối…";
    try {
      const { testProvider } = await import("./settings.js");
      await Promise.all(state.config.providers.map(provider => testProvider(provider.id, false)));
      chooser.replaceChildren();
      const source = el("select", { ariaLabel: "Nguồn AI" });
      for (const provider of state.config.providers) {
        const connected = state.providerStatuses.get(provider.id)?.ok;
        source.append(el("option", { value: provider.id, disabled: !connected,
          textContent: `${provider.name}${connected ? "" : " · chưa kết nối"}` }));
      }
      source.value = state.config.providers.find(provider => state.providerStatuses.get(provider.id)?.ok)?.id || "";
      const model = el("select", { ariaLabel: "Model" });
      const effort = el("select", { ariaLabel: "Mức suy luận" });
      for (const value of EFFORTS) effort.append(el("option", { value, textContent: EFFORT_LABELS[value] }));
      const permission = el("select", { ariaLabel: "Quyền sửa file" });
      permission.append(el("option", { value: "write", textContent: "Được sửa file" }),
        el("option", { value: "read", textContent: "Chỉ đọc file" }));
      const note = el("p", { className: "dc-meta" });
      function models() {
        const provider = state.config.providers.find(item => item.id === source.value);
        model.replaceChildren(...(provider?.models || []).map(value => el("option", { value, textContent: value })));
        effort.hidden = !EFFORT_KINDS.has(provider?.kind);
        effort.value = "default";
        const canWrite = provider?.agent_capable || ["codex_cli", "claude_cli"].includes(provider?.kind);
        permission.hidden = !canWrite;
        note.textContent = canWrite ? "" : "Nguồn này chỉ chat, không sửa file";
        buttons();
      }
      source.onchange = models;
      const send = el("button", { textContent: "Thực hiện", onclick: async () => {
        if (blocked() || !source.value) return;
        busy = true; buttons();
        try {
          const debate = await api(`/api/debates/${encodeURIComponent(getRecord().id)}`);
          const chat = await import("./chat.js");
          if (debate.chat_id && state.chatId !== debate.chat_id) {
            await chat.loadChat(debate.chat_id);
            if (state.chatId !== debate.chat_id) throw new Error("Không mở được chat tranh luận");
          } else if (!debate.chat_id) chat.newChat();
          chat.folderInput.value = debate.folder;
          state.devChat = false;
          state.selectedSource = "provider";
          setupProviderControls(source.value, model.value);
          state.effort = effort.hidden ? "default" : effort.value;
          chat.permissionSelect.value = permission.value;
          const provider = state.config.providers.find(item => item.id === source.value);
          chat.fileToolsSelect.value = provider?.kind === "openai_compat" && provider.agent_capable ? permission.value : "none";
          chat.messageInput.placeholder = "Nhắn tin với AI…";
          renderModePicker(); saveLastMode(); chat.rememberCurrentChat();
          $("#plans-view").hidden = true; $("#self-dev-view").hidden = true; $(".main-panel").hidden = false;
          chooser.replaceChildren();
          const instruction = "Thực hiện phán quyết của cuộc tranh luận dưới đây. Làm từng bước, chạy kiểm tra nếu có, "
            + "cuối cùng báo đã sửa file nào và còn gì chưa làm.";
          await chat.sendMessage({ text: `${instruction}\n\n${debate.plan_request}`, direct: true });
        } catch (error) { showToast(error.message); }
        finally { busy = false; buttons(); }
      } });
      chooser.append(source, model, effort, permission, note, send);
      models();
    } catch (error) { chooser.textContent = error.message; }
    finally { busy = false; buttons(); }
  }
  window.addEventListener("debate-busy", () => { buttons(); if (plan) renderPlan(); });
  window.addEventListener("plan-progress", event => {
    if (event.detail.id === getRecord().plan_id) { plan = event.detail; updateProgress(); }
  });
  // Re-read persisted statuses when a card is reopened, including changes made in the plan editor.
  root.addEventListener("mouseenter", refreshPlan);
  return { el: root, refresh: refreshPlan, update() {
    root.hidden = getRecord().status !== "done" || !getRecord().verdict;
    buttons();
    if (getRecord().plan_id && loadedId !== getRecord().plan_id) refreshPlan();
  } };
}
