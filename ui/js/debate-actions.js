import { $, el, showToast } from "./util.js";
import { api } from "./api.js";
import { state } from "./state.js";
import { EFFORTS, EFFORT_KINDS, EFFORT_LABELS, setupProviderControls, renderModePicker, saveLastMode } from "./mode-chip.js";
import { activePlanRun, planFromRequest, openPlan, persistPlan, loadPlans } from "./plans.js";

// Verdict actions survive transcript updates; execution lives in the linked plan card.
export function createDebateActions(getRecord, onProgress) {
  const root = el("section", { className: "dc-plan" });
  const controls = el("div", { className: "dc-actions" });
  const body = el("div");
  const chooser = el("div", { className: "dc-execute" });
  let plan = null;
  let loadedId = null;
  let busy = false;
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
  async function attachLegacyPlan(saved) {
    const record = getRecord();
    if (saved.chat_id || !record.chat_id) return;
    saved.chat_id = record.chat_id;
    await persistPlan(saved);
    await loadPlans();
    if (state.chatId === record.chat_id && !state.messages.some(message => message.plan_id === saved.id)) {
      const { renderMessages } = await import("./chat.js");
      const { planSummary } = await import("./plan-summary.js");
      const index = state.messages.findIndex(message => message.debate_id === record.id);
      state.messages.splice(index < 0 ? state.messages.length : index + 1, 0,
        { role: "assistant", content: planSummary(saved), plan_id: saved.id });
      renderMessages();
    }
  }
  async function refreshPlan() {
    const id = getRecord().plan_id;
    if (!id || refreshing || busy || activePlanRun?.plan.id === id) return;
    refreshing = true;
    try {
      const saved = await api(`/api/plans/${encodeURIComponent(id)}`);
      if (busy || activePlanRun?.plan.id === id || getRecord().plan_id !== id) return;
      await attachLegacyPlan(saved);
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
  }
  async function makePlan() {
    if (blocked()) return;
    busy = true; buttons(); errorText = "";
    try {
      const debate = await api(`/api/debates/${encodeURIComponent(getRecord().id)}`);
      const chat = await import("./chat.js");
      if (debate.chat_id && state.chatId !== debate.chat_id) await chat.loadChat(debate.chat_id);
      chat.folderInput.value = debate.folder;
      if (debate.plan_id) {
        plan = await api(`/api/plans/${encodeURIComponent(debate.plan_id)}`);
        await attachLegacyPlan(plan);
        await openPlan(plan);
      } else {
        plan ||= await planFromRequest(debate.plan_request || "", { appendRequest: false, afterDebateId: debate.id });
        if (!plan) throw new Error("Không tạo được kế hoạch; hãy thử lại");
        await api(`/api/debates/${encodeURIComponent(debate.id)}/plan`, {
          method: "POST", body: JSON.stringify({ plan_id: plan.id }),
        });
        getRecord().plan_id = plan.id;
      }
      loadedId = plan.id;
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
          $("#self-dev-view").hidden = true; $(".main-panel").hidden = false;
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
