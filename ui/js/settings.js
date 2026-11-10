import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { renderModePicker, renderMutisRoleRows, saveLastMode, setupProviderControls } from "./mode-chip.js";
import { folderInput, messageInput, providerSelect } from "./chat.js";
import { api } from "./api.js";

export async function loadConfig() {
  state.config = await api("/api/config");
  state.orchestration = await api("/api/mutis");
  renderOrchestrationSettings();
  renderModePicker();
  setupProviderControls(state.config.providers[0]?.id);
  folderInput.value = state.config.default_folder || "";
  renderPlannerSettings();
  if (state.selectedSource === "mutis") testAllProviders();
}
export function resolveConnectedRole(key) {
  const current = state.orchestration?.roles?.[key];
  if (current && state.providerStatuses.get(current.provider_id)?.status === "connected") return current;
  const order = { lead: ["claude_cli", "anthropic", "codex_cli"], big: ["codex_cli", "claude_cli"], small: ["deepseek_agent", "codex_cli"] }[key];
  const provider = order.flatMap(kind => state.config.providers.filter(item => item.kind === kind))
    .find(item => state.providerStatuses.get(item.id)?.status === "connected");
  return provider ? { ...current, provider_id: provider.id, model: current?.provider_id === provider.id && current.model ? current.model : (provider.models || [])[0] || "", effort: current?.effort || "default" } : null;
}
export const ROUTE_KINDS = ["small_edit", "code", "test", "docs", "plan", "review"];
export const ASSIGNEE_KINDS = ["lead", "big", "small"];
export function renderOrchestrationSettings() {
  const config = state.orchestration || {};
  renderMutisRoleRows("settings-mutis-roles");
  for (const input of document.querySelectorAll("[data-orch]")) {
    const key = input.dataset.orch;
    if (input.type === "checkbox") input.checked = Boolean(config[key]); else input.value = key === "verify_ignore" ? (config[key] || ["docs/**", "**/*.md"]).join("\n") : (config[key] ?? "");
  }
  const reviewer = $('[data-orch="reviewer_provider_id"]');
  reviewer.replaceChildren(...(state.config.providers || []).map(p => el("option", {value:p.id,textContent:p.name})));
  reviewer.value = config.reviewer_provider_id || "";
  const reviewerProvider = state.config.providers.find(p => p.id === reviewer.value);
  const reviewerModel = $('[data-orch="reviewer_model"]');
  reviewerModel.replaceChildren(...(reviewerProvider?.models || []).map(model => el("option", {value:model,textContent:model})));
  reviewerModel.value = config.reviewer_model || reviewerProvider?.models?.[0] || "";
  const routes = $("#orchestration-routing"); routes.replaceChildren();
  for (const kind of ROUTE_KINDS) {
    const rule = (config.routing || []).find(r => r.kind === kind) || {kind,role:(kind==="small_edit"||kind==="docs"?"small":kind==="plan"||kind==="review"?"lead":"big"),note:""};
    const select = el("select", {dataset:{route:kind}}, ASSIGNEE_KINDS.map(value => el("option", {value,textContent:value})));
    select.value = rule.role || ({deepseek_agent:"small",codex_cli:"big",claude_cli:"lead"}[rule.assignee_kind]||"big");
    routes.append(el("label", {className:"route-row"}, [el("span", {textContent:kind}), select]));
  }
  updateOrchestrationDisabled();
}
export function updateOrchestrationDisabled() {
  const enabled = $('[data-orch="enabled"]').checked;
  if (!enabled && state.selectedSource === "mutis") {
    state.selectedSource = "provider";
    messageInput.placeholder = "Nhắn tin với AI…";
    saveLastMode();
  }
  $("#orchestration-controls").classList.toggle("orchestration-disabled", !enabled);
  for (const input of $("#orchestration-controls").querySelectorAll("input,select")) input.disabled = !enabled;
  $("#orchestration-badge").textContent = `MutiS: ${enabled ? "BẬT" : "TẮT"}`;
  const sourceRow=$("#mode-providers")?.querySelector('[data-source="mutis"]');if(sourceRow){sourceRow.disabled=!enabled;sourceRow.textContent=enabled?"✦ MutiS (3 AI)":"✦ MutiS — bật trong Cài đặt";} renderModePicker();
}
let orchestrationSaveTimer;
export function scheduleOrchestrationSave() {
  clearTimeout(orchestrationSaveTimer);
  orchestrationSaveTimer = setTimeout(saveOrchestration, 180);
}
export async function saveOrchestration() {
  const next = {...state.orchestration};
  for (const input of document.querySelectorAll("[data-orch]")) next[input.dataset.orch] = input.type === "checkbox" ? input.checked : (input.type === "number" ? Number(input.value) : input.dataset.orch === "verify_ignore" ? input.value.split(/\r?\n/).map(x => x.trim()).filter(Boolean) : input.value);
  next.routing = ROUTE_KINDS.map(kind => ({kind,role:$(`[data-route="${kind}"]`).value,note:""}));
  try { state.orchestration = await api("/api/mutis", {method:"PUT",body:JSON.stringify(next)}); updateOrchestrationDisabled(); }
  catch(error) { showToast(error.message); renderOrchestrationSettings(); }
}
export function renderPlannerSettings() {
  const select = $("#planner-provider");
  if (!select) return;
  select.replaceChildren(...(state.config?.providers || []).map(provider =>
    el("option", { value: provider.id, textContent: provider.name })));
  const preferred = state.orchestration?.roles?.lead?.provider_id || state.config?.planner_provider_id || ["claude_cli", "anthropic", "codex_cli"]
    .map(kind => state.config?.providers.find(provider => provider.kind === kind))
    .find(provider => provider && (state.providerStatuses.get(provider.id)?.ok ?? true))?.id
    || state.config?.providers[0]?.id;
  select.value = preferred || "";
  const provider = state.config?.providers.find(item => item.id === select.value);
  $("#planner-model").value = state.orchestration?.roles?.lead?.model || state.config?.planner_model || provider?.models[0] || "";
  $("#planner-model-options").replaceChildren(...(provider?.models || []).map(model => el("option", { value: model })));
}
export async function savePlannerConfig() {
  state.config.planner_provider_id = $("#planner-provider").value;
  state.config.planner_model = $("#planner-model").value.trim();
  await api("/api/config", { method: "PUT", body: JSON.stringify({
    default_folder: state.config.default_folder,
    planner_provider_id: state.config.planner_provider_id,
    planner_model: state.config.planner_model,
  }) });
}
export function clearProviderForm() {
  state.settingsProviderId = null;
  $("#provider-id").value = "";
  $("#provider-name").value = "";
  $("#provider-kind").value = "openai_compat";
  $("#provider-url").value = "";
  $("#provider-key").value = "";
  $("#provider-executable").value = "";
  $("#provider-key").placeholder = "Nhập API key (không bắt buộc với CLI)";
  $("#provider-models").value = "";
  $("#provider-delete").hidden = true;
  $("#provider-form-note").textContent = "";
  $("#cli-candidates").hidden = true;
  updateConnectionLine();
  renderProviderSettingsList();
  updateProviderFormKind();
}
export function renderProviderSettingsList() {
  const root = $("#provider-settings-list");
  root.replaceChildren();
  for (const provider of state.config?.providers || []) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `provider-chip${provider.id === state.settingsProviderId ? " selected" : ""}`;
    const dot = document.createElement("span"); dot.className = "status-dot";
    const status = state.providerStatuses.get(provider.id);
    button.dataset.status = state.providerChecking.has(provider.id) ? "checking" : (status?.status || "unknown");
    button.title = status?.detail || (status?.status === "not_configured" ? "Chưa cấu hình" : "Chưa kiểm tra");
    const label = document.createElement("span"); label.textContent = provider.name;
    button.append(dot, label);
    button.addEventListener("click", () => editProvider(provider.id));
    root.append(button);
  }
}
export function editProvider(id) {
  const provider = state.config.providers.find((item) => item.id === id);
  if (!provider) return;
  state.settingsProviderId = provider.id;
  $("#provider-id").value = provider.id;
  $("#provider-name").value = provider.name;
  $("#provider-kind").value = provider.kind;
  $("#provider-url").value = provider.base_url || "";
  $("#provider-key").value = "";
  $("#provider-executable").value = provider.executable || "";
  $("#provider-key").placeholder = provider.api_key ? "Key đã lưu — để trống để giữ nguyên" : "Nhập API key (không bắt buộc với CLI)";
  $("#provider-models").value = (provider.models || []).join("\n");
  $("#provider-delete").hidden = false;
  $("#provider-form-note").textContent = "";
  $("#cli-candidates").hidden = true;
  renderProviderSettingsList();
  updateProviderFormKind();
  updateConnectionLine();
}
export function updateConnectionLine() {
  const id = state.settingsProviderId;
  const status = state.providerStatuses.get(id);
  const line = $("#provider-connection");
  const busy = state.providerChecking.has(id);
  line.className = `connection-detail ${busy ? "checking" : (status?.status || "unknown")}`;
  line.textContent = busy ? "Đang kiểm tra…" : (status?.message || "Chưa kiểm tra");
  line.title = status?.detail || "";
  $("#provider-test").disabled = !id || busy;
  const provider = state.config?.providers.find((item) => item.id === id);
  $("#provider-login").hidden = provider?.kind !== "claude_cli" || status?.message !== "Claude Code chưa đăng nhập";
}
export async function testProvider(id, toastResult = true) {
  if (!id) return;
  state.providerChecking.add(id); renderProviderSettingsList(); updateConnectionLine(); renderModePicker();
  try {
    const result = await api(`/api/providers/${encodeURIComponent(id)}/test`, { method: "POST" });
    const previous = state.providerStatuses.get(id);
    state.providerStatuses.set(id, result);
    if (toastResult && result.status === "connected" && previous?.status !== "connected") {
      const name = state.config?.providers.find((p) => p.id === id)?.name || id; showToast(`✓ Đã kết nối ${name}`);
    } else if (toastResult && result.status === "error" && previous?.status !== "error") {
      const name = state.config?.providers.find((p) => p.id === id)?.name || id; showToast(`✗ ${name}: ${result.message}`);
    }
  } catch (error) {
    const result = { status: "error", message: "Không kết nối được", detail: error.message };
    state.providerStatuses.set(id, result);
    if (toastResult) showToast(`✗ ${state.config?.providers.find((p) => p.id === id)?.name || id}: ${result.message}`);
  } finally {
    state.providerChecking.delete(id); renderProviderSettingsList(); updateConnectionLine(); renderModePicker();
  }
}
export async function testAllProviders() {
  await Promise.all((state.config?.providers || []).map((p) => testProvider(p.id, false)));
  if (state.selectedSource === "mutis") {
    const resolvedLead = resolveConnectedRole("lead");
    if (resolvedLead && resolvedLead.provider_id !== state.orchestration?.roles?.lead?.provider_id) {
      state.orchestration.roles.lead = resolvedLead;
      try { state.orchestration = await api("/api/mutis", { method: "PUT", body: JSON.stringify(state.orchestration) }); }
      catch (error) { showToast(error.message); }
      renderModePicker();
    }
  }
}
export async function loginClaude() {
  const id = state.settingsProviderId;
  if (!id) return;
  const button = $("#provider-login");
  button.disabled = true;
  $("#provider-form-note").textContent = "Hoàn tất đăng nhập trong trình duyệt rồi bấm Kiểm tra kết nối";
  try {
    await api(`/api/providers/${encodeURIComponent(id)}/login`, { method: "POST" });
    const startedAt = Date.now();
    while (Date.now() - startedAt < 3 * 60 * 1000) {
      await new Promise((resolve) => setTimeout(resolve, 5000));
      await testProvider(id, false);
      if (state.providerStatuses.get(id)?.status === "connected") {
        showToast("✓ Đã kết nối Claude Code CLI");
        break;
      }
    }
  } catch (error) {
    $("#provider-form-note").textContent = error.message;
  } finally {
    button.disabled = false;
  }
}
export function updateProviderFormKind() {
  const kind = $("#provider-kind").value;
  const showUrl = kind === "openai_compat" || kind === "deepseek_agent";
  const showExecutable = kind === "codex_cli" || kind === "claude_cli";
  $("#base-url-field").hidden = !showUrl;
  $("#provider-executable-field").hidden = !showExecutable;
  $("#cli-discover").hidden = !showExecutable;
  $("#models-load").hidden = !showUrl;
  $("#use-deploy-key").hidden = kind !== "deepseek_agent" || !state.settingsProviderId;
}
export async function useDeployKey() {
  if (!state.settingsProviderId) return;
  try {
    state.config = await api(`/api/providers/${encodeURIComponent(state.settingsProviderId)}/use-deploy-key`, {method:"POST"});
    $("#provider-form-note").textContent = "Đã sao chép key phía máy chủ từ Deploy (DeepSeek).";
    editProvider(state.settingsProviderId);
    $("#provider-form-note").textContent = "Đã sao chép key phía máy chủ từ Deploy (DeepSeek).";
  } catch (error) { $("#provider-form-note").textContent = error.message; }
}
export async function saveProvider(event, testAfterSave = true) {
  event.preventDefault();
  const id = state.settingsProviderId;
  const models = $("#provider-models").value.split(/\r?\n/).map((model) => model.trim()).filter(Boolean);
  const body = {
    name: $("#provider-name").value.trim(),
    kind: $("#provider-kind").value,
    base_url: $("#provider-url").value.trim(),
    api_key: $("#provider-key").value,
    executable: $("#provider-executable").value.trim(),
    models,
  };
  try {
    const config = await api(id ? `/api/providers/${encodeURIComponent(id)}` : "/api/providers", {
      method: id ? "PUT" : "POST",
      body: JSON.stringify(body),
    });
    state.config = config;
    if (!id && config.providers.length) state.settingsProviderId = config.providers[config.providers.length - 1].id;
    $("#provider-key").value = "";
    $("#provider-key").placeholder = "Key đã lưu — để trống để giữ nguyên";
    $("#provider-id").value = state.settingsProviderId || "";
    $("#provider-delete").hidden = false;
    $("#provider-form-note").textContent = "Đã lưu nguồn.";
    renderProviderSettingsList();
    setupProviderControls(providerSelect.value || state.settingsProviderId);
    if (testAfterSave) await testProvider(state.settingsProviderId, true);
  } catch (error) {
    $("#provider-form-note").textContent = error.message;
  }
}
export async function deleteProvider() {
  if (!state.settingsProviderId || !window.confirm("Xóa nguồn AI này?")) return;
  try {
    state.config = await api(`/api/providers/${encodeURIComponent(state.settingsProviderId)}`, { method: "DELETE" });
    clearProviderForm();
    setupProviderControls(state.config.providers[0]?.id);
  } catch (error) { $("#provider-form-note").textContent = error.message; }
}
export async function loadModels() {
  try {
    if (!state.settingsProviderId) {
      await saveProvider(new Event("submit", { cancelable: true }));
    } else {
      await saveProvider(new Event("submit", { cancelable: true }));
    }
    if (!state.settingsProviderId) throw new Error("Hãy lưu nguồn trước khi tải model.");
    const result = await api(`/api/providers/${encodeURIComponent(state.settingsProviderId)}/models`);
    $("#provider-models").value = result.models.join("\n");
    state.config.providers = state.config.providers.map((provider) => provider.id === state.settingsProviderId ? { ...provider, models: result.models } : provider);
    $("#provider-form-note").textContent = `Đã tải ${result.models.length} model.`;
  } catch (error) { $("#provider-form-note").textContent = error.message; }
}
export async function refreshCliStatus() {
  const root = $("#cli-status");
  root.replaceChildren();
  const loading = document.createElement("span");
  loading.className = "muted";
  loading.textContent = "Đang kiểm tra…";
  root.append(loading);
  try {
    const status = await api("/api/cli-status");
    root.replaceChildren();
    for (const [key, label] of [["codex_cli", "Codex CLI"], ["claude_cli", "Claude Code"]]) {
      const item = status[key];
      const card = document.createElement("div");
      card.className = "cli-card";
      const title = document.createElement("strong");
      title.textContent = label;
      const detail = document.createElement("span");
      detail.textContent = item.installed ? (item.version || "Đã cài, không đọc được phiên bản") : "Chưa tìm thấy ở các vị trí đã kiểm tra";
      card.append(title, detail);
      if (item.path) {
        const path = document.createElement("span");
        path.className = "cli-path";
        path.textContent = `Đường dẫn: ${item.path}`;
        card.append(path);
      }
      root.append(card);
    }
  } catch (error) { root.textContent = error.message; }
}
export async function discoverCli() {
  const kind = $("#provider-kind").value; const name = kind === "codex_cli" ? "codex" : "claude";
  const button = $("#cli-discover"); const area = $("#cli-candidates");
  button.disabled = true; button.textContent = "Đang tìm…"; area.hidden = true; area.replaceChildren();
  try {
    const candidates = await api(`/api/cli-discover/${name}`);
    const pick = async (candidate) => {
      $("#provider-executable").value = candidate.path;
      area.hidden = true;
      await saveProvider(new Event("submit", { cancelable: true }), false);
      if (state.settingsProviderId) {
        await testProvider(state.settingsProviderId, false);
        const status = state.providerStatuses.get(state.settingsProviderId);
        const cliName = name === "codex" ? "Codex CLI" : "Claude Code CLI";
        if (status?.status === "connected") {
          showToast(`✓ Đã tìm thấy và kết nối ${cliName} (${candidate.version})`);
        } else {
          showToast(`✗ ${cliName}: ${status?.message || "Không kết nối được"}`);
        }
      }
    };
    if (candidates.length === 1) await pick(candidates[0]);
    else if (candidates.length > 1) {
      const title = document.createElement("strong"); title.textContent = "Chọn bản cài đặt:"; area.append(title);
      for (const candidate of candidates) {
        const item = document.createElement("button");
        item.type = "button";
        item.className = "candidate-option";
        item.textContent = `${candidate.path} · ${candidate.version} · ${candidate.source}`;
        item.title = item.textContent;
        item.addEventListener("click", () => pick(candidate));
        area.append(item);
      }
      area.hidden = false;
    } else {
      const hint = name === "codex" ? "npm i -g @openai/codex" : "npm i -g @anthropic-ai/claude-code";
      const text = document.createElement("span"); text.textContent = `Không tìm thấy · ${hint}`;
      const copy = document.createElement("button"); copy.type = "button"; copy.className = "subtle-button"; copy.textContent = "Sao chép"; copy.addEventListener("click", () => navigator.clipboard.writeText(hint));
      area.append(text, copy); area.hidden = false;
    }
  } catch (error) { area.textContent = error.message; area.hidden = false; }
  finally { button.disabled = false; button.textContent = "Tự tìm trên máy"; }
}
export async function saveDefaultFolder() {
  if (!state.config || !folderInput.value.trim()) return;
  state.config.default_folder = folderInput.value.trim();
  try { state.config = await api("/api/config", { method: "PUT", body: JSON.stringify({ default_folder: state.config.default_folder, planner_provider_id: state.config.planner_provider_id || "", planner_model: state.config.planner_model || "" }) }); }
  catch (error) { showToast(error.message); }
}
