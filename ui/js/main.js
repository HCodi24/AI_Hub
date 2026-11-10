import { state } from "./state.js";
import { $, el, showToast } from "./util.js";
import { connectDevEvents, devAction, hideDevPage, refreshDevRepo, renderDevDiff, showDevPage, streamDevRun } from "./dev.js";
import { renderModePicker, renderMutisRoleRows, saveLastMode, saveMutisRoles, updateModels, updatePermissionControl } from "./mode-chip.js";
import {
  stopChat,
  addImageFiles,
  messageInput,
  modelInput,
  newChat,
  openDevChat,
  permissionSelect,
  providerSelect,
  fileToolsSelect,
  refreshChats,
  rememberCurrentChat,
  renderDevChatControls,
  resizeComposer,
  sendMessage,
} from "./chat.js";
import { api, readSse } from "./api.js";
import {
  scheduleMutisSave,
  clearProviderForm,
  confirmClaudeLogin,
  cancelClaudeLogin,
  deleteProvider,
  discoverCli,
  editProvider,
  loadConfig,
  loadModels,
  loginClaude,
  refreshCliStatus,
  renderPlannerSettings,
  saveDefaultFolder,
  saveMutis,
  savePlannerConfig,
  saveProvider,
  testAllProviders,
  testProvider,
  updateMutisDisabled,
  updateProviderFormKind,
} from "./settings.js";
import { loadMutisPage, renderDiagnosis } from "./mutis-page.js";
import { loadPlans, planFromRequest, renderPlanList } from "./plans.js";
import { loadExtensions } from "./extensions.js";

$("#self-dev-open").addEventListener("click",showDevPage);
$("#self-dev-back").addEventListener("click",hideDevPage);
$("#dev-open-chat").addEventListener("click",openDevChat);
$("#dev-open-plan").addEventListener("click",async()=>{await openDevChat();messageInput.value="Sửa AI Hub: ";planFromRequest(messageInput.value);});
$("#dev-checkpoint").addEventListener("click", async () => {
  try {
    const r = await devAction("/api/dev/git/checkpoint", {});
    $("#dev-git-result").textContent = r.skipped ? r.message : "Đã tạo checkpoint " + r.hash;
    await refreshDevRepo();
  } catch (e) {
    showToast(e.message);
  }
});
$("#dev-show-diff").addEventListener("click",async()=>{try{renderDevDiff(await devAction("/api/dev/git/diff"));}catch(e){showToast(e.message)}});
$("#dev-reset").addEventListener("click", async () => {
  try {
    const before = await devAction("/api/dev/git/diff");
    if (!window.confirm("Bỏ thay đổi các file này?\n\n" + before.files.join("\n"))) return;
    const r = await devAction("/api/dev/git/reset", {});
    $("#dev-git-result").textContent = "Đã bỏ thay đổi:\n" + r.files.join("\n");
    await refreshDevRepo();
  } catch (e) {
    showToast(e.message);
  }
});
$("#dev-commit").addEventListener("click", async () => {
  const message = $("#dev-commit-message").value.trim();
  if (!message) {
    showToast("Nhập nội dung commit");
    return;
  }
  try {
    const r = await devAction("/api/dev/git/commit", { message });
    $("#dev-git-result").textContent = "Đã commit " + r.hash;
    $("#dev-commit-message").value = "";
    await refreshDevRepo();
  } catch (e) {
    showToast(e.message);
  }
});
$("#dev-test").addEventListener("click",()=>streamDevRun("/api/dev/test",$("#dev-test-log")));
$("#dev-release").addEventListener("click",()=>streamDevRun("/api/dev/release",$("#dev-release-log")));
$("#extensions-open").addEventListener("click",async()=>{$(".main-panel").hidden=true;$("#extensions-view").hidden=false;try{await loadExtensions()}catch(e){showToast(e.message)}});
$("#mutis-open").addEventListener("click", async () => {
  $(".main-panel").hidden = true;
  $("#extensions-view").hidden = true;
  $("#plans-view").hidden = true;
  $("#mutis-view").hidden = false;
  try {
    await loadMutisPage();
  } catch (e) {
    showToast(e.message);
  }
});
$("#mutis-back").addEventListener("click",()=>{$("#mutis-view").hidden=true;$(".main-panel").hidden=false});
$("#mutis-diagnose").addEventListener("click", async () => {
  const button = $("#mutis-diagnose"), progress = $("#mutis-diagnosis-progress"), log = $("#mutis-diagnosis-log");
  button.disabled = true; progress.hidden = false; log.textContent = "";
  try {
    const response = await fetch("/api/mutis/diagnose", { method: "POST" });
    if (!response.ok) { const body = await response.json(); throw new Error(body.error || `HTTP ${response.status}`); }
    await readSse(response.body, (event, data) => {
      if (event === "log") { log.textContent += `${data.text || JSON.stringify(data)}\n`; log.scrollTop = log.scrollHeight; }
      if (event === "error") { const message = data.message || data.text || "Chẩn đoán thất bại"; log.textContent += `${message}\n`; showToast(message); }
      if (event === "diagnosis") { renderDiagnosis(data); showToast(data.summary || "Đã chẩn đoán"); }
    });
    await loadMutisPage();
  } catch (error) { showToast(error.message); log.textContent += `${error.message}\n`; }
  finally { button.disabled = false; progress.hidden = true; }
});
$("#extensions-back").addEventListener("click",()=>{$("#extensions-view").hidden=true;$(".main-panel").hidden=false});
$("#plans-open").addEventListener("click", async () => {
  $(".main-panel").hidden = true;
  $("#extensions-view").hidden = true;
  $("#plans-view").hidden = false;
  $("#plan-editor").hidden = true;
  try {
    await loadPlans();
  } catch (e) {
    showToast(e.message);
  }
});
$("#plans-back").addEventListener("click",()=>{$("#plans-view").hidden=true;$(".main-panel").hidden=false});
$("#plan-button").addEventListener("click",()=>planFromRequest(messageInput.value));
$("#hint-plan").addEventListener("click",()=>planFromRequest(messageInput.value));
$("#hint-dismiss").addEventListener("click",()=>{$("#plan-hint").hidden=true});
messageInput.addEventListener("input", () => {
  const v = messageInput.value.toLowerCase();
  const suggest = v.length > 300 || ["làm app", "xây dựng", "thêm tính năng", "refactor"].some(x => v.includes(x));
  $("#plan-hint").hidden = !suggest;
});
$("#planner-provider").addEventListener("change", () => {
  const p = state.config.providers.find(x => x.id === $("#planner-provider").value);
  $("#planner-model").value = p?.models[0] || "";
  renderPlannerSettings();
  savePlannerConfig().catch(e => showToast(e.message));
});
$("#planner-model").addEventListener("change",()=>savePlannerConfig().catch(e=>showToast(e.message)));
$("#chat-idle-timeout").addEventListener("change", () => {
  const input = $("#chat-idle-timeout");
  input.value = String(Math.max(0, Math.min(120, Number(input.value) || 0)));
  savePlannerConfig().then(() => { state.config.chat_idle_timeout_minutes = Number(input.value); })
    .catch(e => showToast(e.message));
});
$("#extension-stop").addEventListener("click",()=>fetch("/api/extensions/codegraph/stop",{method:"POST"}));

$("#new-chat").addEventListener("click", newChat);
$("#dev-chat-new").addEventListener("click", openDevChat);
$("#settings-open").addEventListener("click", async () => {
  $("#settings-dialog").showModal();
  renderPlannerSettings();
  if (state.settingsProviderId) editProvider(state.settingsProviderId);
  else clearProviderForm();
  testAllProviders();
  await refreshCliStatus();
});
$("#dev-auto-rebuild").addEventListener("change", async () => {
  try { const result=await api("/api/dev/settings",{method:"PUT",body:JSON.stringify({auto_rebuild:$("#dev-auto-rebuild").checked})}); state.devAutoRebuild=result.auto_rebuild; }
  catch(error) { showToast(error.message); }
});
$("#provider-select").addEventListener("change", () => {
  state.selectedSource = "provider";
  messageInput.placeholder = "Nhắn tin với AI…";
  updateModels();
  updatePermissionControl();
  rememberCurrentChat();
  saveLastMode();
});
$("#model-input").addEventListener("input", () => { renderModePicker(); rememberCurrentChat(); saveLastMode(); });
$("#mode-chip").addEventListener("click", () => {
  const pop = $("#mode-popover"); pop.hidden = !pop.hidden;
  $("#mode-chip").setAttribute("aria-expanded", String(!pop.hidden));
  if (!pop.hidden) {
    pop.querySelector("button:not([disabled])")?.focus();
    testAllProviders();
  }
});
$("#mode-popover").addEventListener("click", (event) => {
  const source=event.target.closest("[data-source]")?.dataset.source;
  if (source === "mutis" && state.mutis?.enabled) {
    state.selectedSource = "mutis";
    messageInput.placeholder = "Nhắn tin với MutiS — bấm ✦ Gợi ý chia việc để chia việc cho 3 AI…";
    renderModePicker();
    saveLastMode();
    const section = $("#mode-mutis-section");
    section.hidden = false;
    section.scrollIntoView({ block: "nearest", behavior: "smooth" });
    testAllProviders();
    return;
  }
  const providerId = event.target.closest("[data-provider]")?.dataset.provider;
  const model = event.target.closest("[data-model]")?.dataset.model;
  const effort = event.target.closest("[data-effort]")?.dataset.effort;
  if (providerId) { state.selectedSource="provider";messageInput.placeholder="Nhắn tin với AI…";providerSelect.value = providerId; updateModels(); updatePermissionControl(); }
  if (model) modelInput.value = model;
  if (effort) state.effort = effort;
  if (providerId || model || effort) { renderModePicker(); rememberCurrentChat(); saveLastMode(); }
  if (providerId || model) { $("#mode-popover").hidden = true; $("#mode-chip").setAttribute("aria-expanded", "false"); $("#mode-chip").focus(); }
});
$("#mode-popover").addEventListener("change", (event) => {
  const provider = event.target.closest("[data-role-provider]");
  if (provider) {
    const key = provider.dataset.roleProvider;
    const selected = state.config.providers.find(item => item.id === provider.value);
    state.mutis.roles[key].provider_id = provider.value;
    state.mutis.roles[key].model = selected?.models?.[0] || "";
    renderMutisRoleRows("mode-mutis-roles");
    saveMutisRoles("mode-mutis-section").catch(error => showToast(error.message));
  } else if (event.target.matches("[data-role-model], [data-role-effort], [data-mutis-switch]")) {
    saveMutisRoles("mode-mutis-section").catch(error => showToast(error.message));
  }
});
$("#settings-mutis-roles").addEventListener("change", () => saveMutisRoles("settings-mutis-roles").catch(error => showToast(error.message)));
$("#mode-mutis-roles").addEventListener("change", () => saveMutisRoles("mode-mutis-section").catch(error => showToast(error.message)));
$("#mode-mutis-switches").addEventListener("change", () => saveMutisRoles("mode-mutis-section").catch(error => showToast(error.message)));
$("#mode-other").addEventListener("click", () => {
  const model = window.prompt("Nhập mã model", modelInput.value);
  if (model?.trim()) { modelInput.value = model.trim(); renderModePicker(); rememberCurrentChat(); saveLastMode(); }
});
document.addEventListener("keydown", (event) => {
  const pop = $("#mode-popover");
  if (pop.hidden) return;
  if (event.key === "Escape") { pop.hidden = true; $("#mode-chip").setAttribute("aria-expanded", "false"); $("#mode-chip").focus(); }
  else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    const items = [...pop.querySelectorAll("button:not([disabled])")]; const index = items.indexOf(document.activeElement);
    if (index >= 0) { event.preventDefault(); items[(index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus(); }
  }
});
  document.addEventListener("click", event => {
    const pop = $("#mode-popover");
    if (!document.contains(event.target)) return;
    if (!pop.hidden && !event.target.closest(".mode-picker")) {
      pop.hidden = true;
      $("#mode-chip").setAttribute("aria-expanded", "false");
    }
  });
$("#composer").addEventListener("submit", (event) => { event.preventDefault(); sendMessage(); });
$("#stop-button").addEventListener("click", stopChat);
$("#message-input").addEventListener("input", resizeComposer);
$("#attachment-button").addEventListener("click", () => $("#attachment-input").click());
$("#attachment-input").addEventListener("change", event => addImageFiles(event.target.files));
$("#message-input").addEventListener("paste", event => {
  const files = [...(event.clipboardData?.items || [])].filter(item => item.kind === "file").map(item => item.getAsFile()).filter(Boolean);
  if (files.length) { event.preventDefault(); addImageFiles(files); }
});
$("#composer").addEventListener("dragover", event => { event.preventDefault(); event.dataTransfer.dropEffect = "copy"; });
$("#composer").addEventListener("drop", event => {
  event.preventDefault();
  if (event.dataTransfer.files.length) addImageFiles(event.dataTransfer.files);
});
$("#message-input").addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    sendMessage();
  }
});
$("#folder-input").addEventListener("input", () => { renderDevChatControls(); rememberCurrentChat(); });
permissionSelect.addEventListener("change", rememberCurrentChat);
$("#folder-input").addEventListener("change", saveDefaultFolder);
fileToolsSelect.addEventListener("change", rememberCurrentChat);
$("#provider-add").addEventListener("click", clearProviderForm);
$("#provider-kind").addEventListener("change", updateProviderFormKind);
$("#provider-form").addEventListener("submit", saveProvider);
$("#provider-delete").addEventListener("click", deleteProvider);
$("#provider-test").addEventListener("click", () => testProvider(state.settingsProviderId));
$("#provider-login").addEventListener("click", loginClaude);
$("#claude-login-submit").addEventListener("click", confirmClaudeLogin);
$("#claude-login-code").addEventListener("keydown", (event) => { if (event.key === "Enter") confirmClaudeLogin(); });
$("#claude-login-cancel").addEventListener("click", cancelClaudeLogin);
$("#claude-login-retry").addEventListener("click", loginClaude);
$("#claude-login-copy").addEventListener("click", async () => {
  try { await navigator.clipboard.writeText($("#claude-login-link").href); showToast("Đã sao chép link"); }
  catch (_) { showToast("Không thể truy cập clipboard"); }
});
$("#cli-discover").addEventListener("click", discoverCli);
$("#models-load").addEventListener("click", loadModels);
$("#cli-refresh").addEventListener("click", refreshCliStatus);
document.querySelectorAll("[data-orch]").forEach(input => input.addEventListener("change", () => {
  updateMutisDisabled(); scheduleMutisSave();
}));
$("#mutis-routing").addEventListener("change", () => { scheduleMutisSave(); });
$('[data-orch="reviewer_provider_id"]').addEventListener("change", () => {
  const provider = state.config.providers.find(p => p.id === $('[data-orch="reviewer_provider_id"]').value);
  const model = $('[data-orch="reviewer_model"]');
  model.replaceChildren(...(provider?.models || []).map(value => el("option", {value,textContent:value})));
  saveMutis();
});

(async function init() {
  try {
    try {
      const dev = await api("/api/dev/status");
      state.dev = dev.dev;
      $("#dev-badge").hidden = !state.dev;
      $("#self-dev-open").hidden = !state.dev;
      $("#self-dev-settings").hidden = !state.dev;
      $("#dev-chat-new").hidden = !state.dev;
      if (state.dev) {
        const settings = await api("/api/dev/settings");
        state.devAutoRebuild = settings.auto_rebuild;
        $("#dev-auto-rebuild").checked = state.devAutoRebuild;
        connectDevEvents();
      }
      renderDevChatControls();
    } catch (_) {}
    await loadConfig();
    renderModePicker();
    await refreshChats();
    newChat();
  } catch (error) {
    showToast(error.message);
  }
})();
