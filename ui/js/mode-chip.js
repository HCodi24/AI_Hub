import { $, el } from "./util.js";
import { state } from "./state.js";
import { modelInput, modelOptions, providerSelect } from "./chat.js";
import { api } from "./api.js";

export const EFFORT_KINDS = new Set(["codex_cli", "claude_cli", "openai_compat", "anthropic"]);
export const EFFORT_LABELS = { default: "Mặc định", low: "Thấp", medium: "Vừa", high: "Cao", xhigh: "Rất cao", max: "Max" };
export const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"];
export function currentProvider() {
  return state.config?.providers.find((provider) => provider.id === providerSelect.value);
}
// Old plans/chats may still reference the removed "DeepSeek làm việc" source: it now lives in the Deploy source
// (OpenAI-compatible with agent mode), so map the old id to it.
export function findProvider(id) {
  const providers = state.config?.providers || [];
  return providers.find(item => item.id === id)
    || (id === "deepseek-agent-deploy"
      ? providers.find(item => item.kind === "openai_compat" && item.agent_capable)
      : undefined);
}
export function setupProviderControls(providerId, preferredModel = "") {
  if (!state.config) return;
  providerSelect.replaceChildren();
  for (const provider of state.config.providers) {
    const option = document.createElement("option");
    option.value = provider.id;
    option.textContent = provider.name;
    providerSelect.append(option);
  }
  const selected = findProvider(providerId) || state.config.providers[0];
  if (!selected) {
    updatePermissionControl();
    return;
  }
  providerSelect.value = selected.id;
  updateModels(preferredModel || selected.models[0] || "");
  updatePermissionControl();
}
export function updatePermissionControl() {
  const provider = currentProvider();
  $("#permission-control").hidden = !["codex_cli", "claude_cli"].includes(provider?.kind);
  // OpenAI-compatible sources with agent mode enabled can read/edit files in the chat folder.
  $("#file-tools-control").hidden = !(provider?.kind === "openai_compat" && provider.agent_capable);
}
export function updateModels(preferredModel = "") {
  const provider = currentProvider();
  modelOptions.replaceChildren();
  for (const model of provider?.models || []) {
    const option = document.createElement("option");
    option.value = model;
    modelOptions.append(option);
  }
  modelInput.value = preferredModel || provider?.models?.[0] || "";
  renderModePicker();
}
export function prettyModel(id) {
  const raw = String(id || "").split("/").pop().replace(/-\d{4}(?:\d{4})?$/, "");
  const tokens = raw.split(/[-_]+/).filter(Boolean);
  const words = [];
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (/^\d$/.test(token) && words.length && /^\d(?:\.\d+)?$/.test(words[words.length - 1])) words[words.length - 1] += `.${token}`;
    else if (/^\d$/.test(token) && words.length && /[a-z]$/i.test(words[words.length - 1])) {
      let version = token;
      while (/^\d$/.test(tokens[i + 1] || "")) version += "." + tokens[++i];
      words.push(version);
    }
    else if (/^[a-z]\d+(?:\.\d+)*$/i.test(token)) words.push(token[0].toUpperCase() + token.slice(1));
    else words.push(token.toLowerCase());
  }
  let result = words.map((word, index) => {
    if (["gpt", "ai", "api"].includes(word.toLowerCase())) return word.toUpperCase();
    return word[0] ? word[0].toUpperCase() + word.slice(1) : word;
  });
  for (let i = 0; i < result.length - 1; i++) if (result[i] === "GPT" && /^\d+$/.test(result[i + 1])) result[i + 1] = `-${result[i + 1]}`;
  return result.join(" ").replace(/\s(-\d+)/g, "$1");
}
export function renderModePicker() {
  const provider = currentProvider();
  const chip = $("#mode-chip");
  if (!chip) return;
  const isMutis = state.selectedSource === "mutis";
  const shortName = provider?.name?.split(/\s*\(/)[0] || "";
  chip.classList.toggle("mutis-chip", isMutis);
  $("#mode-chip-provider").textContent = isMutis ? "MutiS · " : (shortName && provider.name !== shortName ? `${shortName} · ` : "");
  if (isMutis) {
    const roles = state.mutis?.roles || {};
    const roleNames = ["lead", "big", "small"].map(key => prettyModel(roles[key]?.model || "—"));
    $("#mode-chip-model").textContent = roleNames.join(" · ");
    $("#mode-chip-effort").textContent = "";
    chip.title = ["lead", "big", "small"].map(key => {
      const role = roles[key] || {};
      const selected = state.config.providers.find(item => item.id === role.provider_id);
      return `${key} → ${selected?.name || role.provider_id} · ${role.model} · ${role.effort}`;
    }).join("\n");
    chip.dataset.status = "connected";
    const dots = $("#mode-status-dots");
    dots.replaceChildren(...["lead", "big", "small"].map(key => {
      const id = roles[key]?.provider_id;
      const status = state.providerStatuses.get(id);
      const color = state.providerChecking.has(id) ? "checking" : status?.ok ? "connected" : status?.status || "unknown";
      return el("i", { className: `status-dot ${color}`, title: status?.detail || "Chưa kiểm tra" });
    }));
    $("#mode-models").closest("section").hidden = true;
    $("#mode-effort-section").hidden = true;
    $("#mode-mutis-section").hidden = false;
    renderMutisRoleRows("mode-mutis-roles"); renderMutisSwitches("mode-mutis-switches");
  } else {
    $("#mode-chip-provider").textContent = shortName && provider?.name !== shortName ? `${shortName} · ` : "";
    $("#mode-chip-model").textContent = prettyModel(modelInput.value);
    $("#mode-models").closest("section").hidden = false;
    $("#mode-mutis-section").hidden = true;
  }
  const supported = EFFORT_KINDS.has(provider?.kind);
  $("#mode-chip-effort").textContent = supported ? ` ${EFFORT_LABELS[state.effort] || EFFORT_LABELS.default}` : "";
  const providers = $("#mode-providers");
  providers.replaceChildren();
  const mutisRow = document.createElement("button");
  mutisRow.type = "button";
  mutisRow.className = "mode-row";
  mutisRow.setAttribute("role", "radio");
  mutisRow.setAttribute("aria-checked", String(isMutis));
  mutisRow.dataset.source = "mutis";
  mutisRow.textContent = state.mutis?.enabled ? "✦ MutiS (3 AI)" : "✦ MutiS — bật trong Cài đặt";
  mutisRow.disabled = !state.mutis?.enabled;
  providers.append(mutisRow);
  for (const item of state.config?.providers || []) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "mode-row";
    row.setAttribute("role", "radio");
    row.setAttribute("aria-checked", String(!isMutis && item.id === provider?.id));
    row.dataset.provider = item.id;
    row.textContent = item.name;
    providers.append(row);
  }
  if (isMutis) return;
  chip.title = modelInput.value;
  const status = state.providerStatuses.get(provider?.id);
  $("#mode-status-dots").replaceChildren(el("i",{className:"status-dot"}));
  chip.dataset.status = state.providerChecking.has(provider?.id) ? "checking" : (status?.status || "unknown");
  chip.title = `${modelInput.value}${status ? ` · ${status.detail}` : " · Chưa kiểm tra"}`;
  $("#mode-effort-section").hidden = !supported;
  const models = $("#mode-models"); models.replaceChildren();
  for (const model of provider?.models || []) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "mode-row";
    row.setAttribute("role", "radio");
    row.setAttribute("aria-checked", String(model === modelInput.value));
    row.dataset.model = model;
    row.textContent = prettyModel(model);
    row.title = model;
    models.append(row);
  }
  const efforts = $("#mode-efforts"); efforts.replaceChildren();
  for (const effort of EFFORTS) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "effort-button";
    row.dataset.effort = effort;
    row.setAttribute("aria-pressed", String(effort === state.effort));
    row.textContent = EFFORT_LABELS[effort];
    efforts.append(row);
  }
}
export function renderMutisRoleRows(containerId) {
  const root = $(`#${containerId}`);
  if (!root) return;
  root.replaceChildren();
  const roleDefinitions = [
    ["lead", "Lập kế hoạch & review", "✦"],
    ["big", "Làm việc lớn", "▣"],
    ["small", "Làm việc nhỏ", "◦"],
  ];
  for (const [key, label, icon] of roleDefinitions) {
    const role = state.mutis.roles?.[key] || { provider_id: "", model: "", effort: "default" };
    const row = el("div", { className: "mutis-role-row" });
    const provider = el("select", { dataset: { roleProvider: key }, ariaLabel: `Nguồn ${label}` });
    provider.append(...(state.config.providers || []).map(item => el("option", { value: item.id, textContent: item.name })));
    provider.value = role.provider_id;

    const selected = state.config.providers.find(item => item.id === role.provider_id);
    const model = el("select", { dataset: { roleModel: key }, ariaLabel: `Model ${label}` });
    model.append(...(selected?.models || []).map(value => el("option", { value, textContent: prettyModel(value) })));
    if (role.model && !(selected?.models || []).includes(role.model)) {
      model.append(el("option", { value: role.model, textContent: prettyModel(role.model) }));
    }
    model.append(el("option", { value: "__other", textContent: "Model khác…" }));
    model.value = role.model || "__other";
    row.append(el("span", { textContent: `${icon} ${label}` }), provider, model);

    if (EFFORT_KINDS.has(selected?.kind)) {
      const effort = el("select", { dataset: { roleEffort: key }, ariaLabel: `Mức suy luận ${label}` });
      effort.append(...EFFORTS.map(value => el("option", { value, textContent: EFFORT_LABELS[value] })));
      effort.value = role.effort || "default";
      row.append(effort);
    }

    const status = state.providerStatuses.get(role.provider_id);
    const dot = el("span", {
      className: `status-dot ${state.providerChecking.has(role.provider_id) ? "checking" : status?.ok ? "connected" : status?.status || "unknown"}`,
      title: state.providerChecking.has(role.provider_id) ? "Đang kiểm tra…" : status?.detail || "Chưa kiểm tra",
    });
    row.append(dot);
    if (!status?.ok) {
      if (!status) {
        row.append(el("small", { className: "muted-inline", textContent: "Đang kiểm tra kết nối…" }));
      } else if (status && status.status !== "connected") {
        const fallbackKinds = { lead: ["claude_cli", "anthropic", "codex_cli"], big: ["codex_cli", "claude_cli"], small: ["codex_cli", "claude_cli"] }[key];
        const agentFirst = key === "small" ? state.config.providers.filter(item => item.agent_capable) : [];
        const fallback = agentFirst.concat(fallbackKinds.flatMap(kind => state.config.providers.filter(item => item.kind === kind)))
          .find(item => item.id !== role.provider_id && state.providerStatuses.get(item.id)?.status === "connected");
        row.append(el("small", { textContent: fallback ? `⚠ chưa kết nối — sẽ dùng ${fallback.name}` : "⚠ Không có nguồn nào đang kết nối cho vai trò này" }));
      }
    }
    root.append(row);
  }
}
export function renderMutisSwitches(containerId) {
  const root = $(`#${containerId}`);
  if (!root) return;
  root.replaceChildren();
  const switches = [["auto_plan", "Tự động chia việc"], ["verify_gate", "Kiểm tra tự động"], ["auto_review", "Claude review"], ["auto_fix", "Tự sửa lỗi"]];
  for (const [key, label] of switches) {
    const input = el("input", {
      type: "checkbox",
      checked: Boolean(state.mutis?.[key]),
      dataset: { mutisSwitch: key },
    });
    root.append(el("label", {}, [input, document.createTextNode(` ${label}`)]));
  }
}
export async function saveMutisRoles(containerId = "mode-mutis-section") {
  const root = $(`#${containerId}`);
  if (!root) return;
  const next = { ...state.mutis, roles: { ...state.mutis.roles } };
  for (const key of ["lead", "big", "small"]) {
    const role = { ...next.roles[key] };
    const provider = root.querySelector(`[data-role-provider="${key}"]`);
    const model = root.querySelector(`[data-role-model="${key}"]`);
    const effort = root.querySelector(`[data-role-effort="${key}"]`);
    const customModel = model?.value === "__other";
    if (provider) role.provider_id = provider.value;
    const selectedProvider = state.config.providers.find(item => item.id === role.provider_id);
    if (model) {
      if (model.value === "__other") {
        role.model = window.prompt("Nhập mã model", role.model) || role.model;
      } else {
        role.model = model.value;
      }
    }
    if (!customModel && !(selectedProvider?.models || []).includes(role.model)) {
      role.model = selectedProvider?.models?.[0] || role.model;
    }
    if (effort) role.effort = effort.value;
    next.roles[key] = role;
  }
  for (const input of root.querySelectorAll("[data-mutis-switch]")) {
    next[input.dataset.mutisSwitch] = input.checked;
  }
  state.mutis = await api("/api/mutis", { method: "PUT", body: JSON.stringify(next) });
  renderModePicker();
  renderMutisRoleRows("settings-mutis-roles");
}
export function saveLastMode() {
  try {
    const value = {
      source: state.selectedSource,
      provider_id: providerSelect.value,
      model: modelInput.value,
      effort: state.effort,
    };
    localStorage.setItem("ai-hub-last-mode", JSON.stringify(value));
    if (state.chatId) {
      localStorage.setItem(`ai-hub-last-mode-${state.chatId}`, JSON.stringify(value));
    }
  } catch (_) { /* Storage may be disabled. */ }
}
