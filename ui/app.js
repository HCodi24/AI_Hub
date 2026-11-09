"use strict";

const $ = (selector) => document.querySelector(selector);
const state = {
  config: null,
  chats: [],
  memoryChats: new Map(),
  memoryData: new Map(),
  chatId: null,
  messages: [],
  streaming: false,
  settingsProviderId: null,
  toastTimer: null,
  effort: "default",
  providerStatuses: new Map(),
  providerChecking: new Set(),
  plans: [], currentPlan: null, planRunning: false,
};

const EFFORT_KINDS = new Set(["codex_cli", "openai_compat", "anthropic"]);
const EFFORT_LABELS = { default: "Mặc định", low: "Thấp", medium: "Vừa", high: "Cao", xhigh: "Rất cao", max: "Max" };
const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"];

const providerSelect = $("#provider-select");
const modelInput = $("#model-input");
const modelOptions = $("#model-options");
const folderInput = $("#folder-input");
const permissionSelect = $("#permission-select");
const messagesRoot = $("#messages");
const welcome = $("#welcome");
const messageInput = $("#message-input");

async function api(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: { ...(options.body ? { "Content-Type": "application/json" } : {}), ...(options.headers || {}) },
  });
  if (!response.ok) {
    let message = `HTTP ${response.status}`;
    try {
      const body = await response.json();
      message = body.error || message;
    } catch (_) { /* Keep the HTTP fallback. */ }
    throw new Error(message);
  }
  if (response.status === 204) return null;
  const type = response.headers.get("content-type") || "";
  return type.includes("application/json") ? response.json() : response;
}

function showToast(message) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.add("visible");
  clearTimeout(state.toastTimer);
  state.toastTimer = setTimeout(() => toast.classList.remove("visible"), 3600);
}

function currentProvider() {
  return state.config?.providers.find((provider) => provider.id === providerSelect.value);
}

function setupProviderControls(providerId, preferredModel = "") {
  if (!state.config) return;
  providerSelect.replaceChildren();
  for (const provider of state.config.providers) {
    const option = document.createElement("option");
    option.value = provider.id;
    option.textContent = provider.name;
    providerSelect.append(option);
  }
  const selected = state.config.providers.find((provider) => provider.id === providerId) || state.config.providers[0];
  if (!selected) {
    updatePermissionControl();
    return;
  }
  providerSelect.value = selected.id;
  updateModels(preferredModel || selected.models[0] || "");
  updatePermissionControl();
}

function updatePermissionControl() {
  const kind = currentProvider()?.kind;
  $("#permission-control").hidden = !["codex_cli", "claude_cli"].includes(kind);
}

function updateModels(preferredModel = "") {
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

function prettyModel(id) {
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

function renderModePicker() {
  const provider = currentProvider();
  const chip = $("#mode-chip");
  if (!chip) return;
  const shortName = provider?.name?.split(/\s*\(/)[0] || "";
  $("#mode-chip-provider").textContent = shortName && provider.name !== shortName ? `${shortName} · ` : "";
  $("#mode-chip-model").textContent = prettyModel(modelInput.value);
  const supported = EFFORT_KINDS.has(provider?.kind);
  $("#mode-chip-effort").textContent = supported ? ` ${EFFORT_LABELS[state.effort] || EFFORT_LABELS.default}` : "";
  chip.title = modelInput.value;
  const status = state.providerStatuses.get(provider?.id);
  chip.dataset.status = state.providerChecking.has(provider?.id) ? "checking" : (status?.status || "unknown");
  chip.title = `${modelInput.value}${status ? ` · ${status.detail}` : " · Chưa kiểm tra"}`;
  $("#mode-effort-section").hidden = !supported;
  const providers = $("#mode-providers"); providers.replaceChildren();
  for (const item of state.config?.providers || []) {
    const row = document.createElement("button"); row.type = "button"; row.className = "mode-row"; row.setAttribute("role", "radio"); row.setAttribute("aria-checked", String(item.id === provider?.id)); row.dataset.provider = item.id; row.textContent = item.name;
    providers.append(row);
  }
  const models = $("#mode-models"); models.replaceChildren();
  for (const model of provider?.models || []) {
    const row = document.createElement("button"); row.type = "button"; row.className = "mode-row"; row.setAttribute("role", "radio"); row.setAttribute("aria-checked", String(model === modelInput.value)); row.dataset.model = model; row.textContent = prettyModel(model); row.title = model;
    models.append(row);
  }
  const efforts = $("#mode-efforts"); efforts.replaceChildren();
  for (const effort of EFFORTS) {
    const row = document.createElement("button"); row.type = "button"; row.className = "effort-button"; row.dataset.effort = effort; row.setAttribute("aria-pressed", String(effort === state.effort)); row.textContent = EFFORT_LABELS[effort]; efforts.append(row);
  }
}

function saveLastMode() {
  try { localStorage.setItem("ai-hub-last-mode", JSON.stringify({ provider_id: providerSelect.value, model: modelInput.value, effort: state.effort })); } catch (_) { /* Storage may be disabled. */ }
}

function newChat() {
  if (state.streaming) return;
  state.chatId = crypto.randomUUID();
  state.messages = [];
  state.memoryChats.set(state.chatId, { id: state.chatId, title: "Chat mới", updated_at: Date.now() / 1000 });
  let lastMode = null;
  try { lastMode = JSON.parse(localStorage.getItem("ai-hub-last-mode") || "null"); } catch (_) { /* Ignore invalid or unavailable storage. */ }
  if (!state.config?.providers.some((provider) => provider.id === lastMode?.provider_id)) lastMode = null;
  setupProviderControls(lastMode?.provider_id || providerSelect.value || state.config?.providers[0]?.id, lastMode?.model || "");
  state.effort = EFFORTS.includes(lastMode?.effort) ? lastMode.effort : "default";
  renderModePicker();
  permissionSelect.value = "write";
  folderInput.value = state.config?.default_folder || "";
  rememberCurrentChat();
  renderMessages();
  renderChatList();
  messageInput.focus();
}

async function loadChat(id) {
  if (state.streaming) return;
  const saved = state.chats.some((chat) => chat.id === id);
  const cached = state.memoryData.get(id);
  if (!saved && cached) {
    state.chatId = id;
    state.messages = cached.messages.map((message) => ({ ...message, logs: [...(message.logs || [])] }));
    setupProviderControls(cached.provider_id, cached.model);
    permissionSelect.value = cached.permission || "write";
    state.effort = cached.effort || "default";
    renderModePicker();
    folderInput.value = cached.folder || state.config?.default_folder || "";
    renderMessages();
    renderChatList();
    return;
  }
  try {
    const chat = await api(`/api/chats/${encodeURIComponent(id)}`);
    state.chatId = chat.id;
    state.messages = chat.messages || [];
    state.memoryChats.set(chat.id, { id: chat.id, title: chat.title, updated_at: chat.updated_at });
    setupProviderControls(chat.provider_id, chat.model);
    permissionSelect.value = chat.permission || "write";
    state.effort = chat.effort || "default";
    renderModePicker();
    folderInput.value = chat.folder || state.config?.default_folder || "";
    renderMessages();
    renderChatList();
  } catch (error) {
    showToast(error.message);
  }
}

async function refreshChats() {
  try {
    state.chats = await api("/api/chats");
    renderChatList();
  } catch (error) {
    showToast(error.message);
  }
}

function renderChatList() {
  const root = $("#chat-list");
  root.replaceChildren();
  const entries = new Map();
  for (const chat of state.chats) entries.set(chat.id, chat);
  for (const chat of state.memoryChats.values()) entries.set(chat.id, chat);
  const sorted = [...entries.values()].sort((a, b) => b.updated_at - a.updated_at);
  for (const chat of sorted) {
    const item = document.createElement("div");
    item.className = `chat-item${chat.id === state.chatId ? " active" : ""}`;
    const select = document.createElement("button");
    select.className = "chat-select";
    select.type = "button";
    select.disabled = state.streaming;
    select.textContent = chat.title || "Chat mới";
    select.title = select.textContent;
    select.addEventListener("click", () => loadChat(chat.id));
    const remove = document.createElement("button");
    remove.className = "chat-delete";
    remove.type = "button";
    remove.disabled = state.streaming;
    remove.textContent = "×";
    remove.title = "Xóa chat";
    remove.setAttribute("aria-label", "Xóa chat");
    remove.addEventListener("click", async () => {
      try {
        await api(`/api/chats/${encodeURIComponent(chat.id)}`, { method: "DELETE" });
        state.memoryChats.delete(chat.id);
        state.memoryData.delete(chat.id);
        if (state.chatId === chat.id) newChat();
        await refreshChats();
      } catch (error) { showToast(error.message); }
    });
    item.append(select, remove);
    root.append(item);
  }
}

function renderMessages() {
  rememberCurrentChat();
  messagesRoot.querySelectorAll(".message").forEach((node) => node.remove());
  welcome.hidden = state.messages.length > 0;
  for (const message of state.messages) {
    const row = document.createElement("article");
    row.className = `message ${message.role === "user" ? "user" : "assistant"}`;
    const body = document.createElement("div");
    body.className = "message-body";
    const tokenLog = message.role === "assistant"
      ? [...(message.logs || [])].reverse().find((entry) => entry.startsWith("Token:"))
      : null;
    if (message.role === "assistant") {
      if (message.reasoning) appendFoldout(body, "Đang suy nghĩ", message.reasoning);
      if (message.logs?.length) appendActivityFoldout(body, message.logs, tokenLog);
      if (message.error) {
        const error = document.createElement("div");
        error.className = "error-text";
        error.textContent = message.error;
        body.append(error);
      }
    }
    renderMarkdown(body, message.content || "");
    if (tokenLog) {
      const usage = document.createElement("div");
      usage.className = "token-usage";
      usage.textContent = tokenLog;
      body.append(usage);
    }
    row.append(body);
    messagesRoot.append(row);
  }
  messagesRoot.scrollTop = messagesRoot.scrollHeight;
}

function rememberCurrentChat() {
  if (!state.chatId) return;
  state.memoryData.set(state.chatId, {
    provider_id: providerSelect.value,
    model: modelInput.value,
    folder: folderInput.value,
    permission: permissionSelect.value || "write",
    effort: state.effort,
    messages: state.messages.map((message) => ({ ...message, logs: [...(message.logs || [])] })),
  });
}

function appendFoldout(parent, title, text) {
  const details = document.createElement("details");
  details.className = "foldout";
  const summary = document.createElement("summary");
  summary.textContent = title;
  const content = document.createElement("div");
  content.className = "foldout-content";
  content.textContent = text;
  details.append(summary, content);
  parent.append(details);
}

function appendActivityFoldout(parent, logs, tokenLog) {
  const details = document.createElement("details");
  details.className = "foldout activity-foldout";
  const summary = document.createElement("summary");
  const label = document.createElement("span");
  label.textContent = "Hoạt động";
  summary.append(label);
  if (tokenLog) {
    const preview = document.createElement("span");
    preview.className = "activity-preview";
    preview.textContent = tokenLog;
    preview.title = tokenLog;
    summary.append(preview);
  }
  const content = document.createElement("div");
  content.className = "foldout-content";
  content.textContent = logs.join("\n");
  details.append(summary, content);
  parent.append(details);
}

function renderMarkdown(parent, source) {
  if (!source) return;
  const codePattern = /```([^\n`]*)\n([\s\S]*?)```/g;
  let last = 0;
  let match;
  while ((match = codePattern.exec(source)) !== null) {
    appendParagraphs(parent, source.slice(last, match.index));
    appendCodeBlock(parent, match[1].trim(), match[2]);
    last = codePattern.lastIndex;
  }
  appendParagraphs(parent, source.slice(last));
}

function appendCodeBlock(parent, language, code) {
  const wrap = document.createElement("div");
  wrap.className = "code-wrap";
  const head = document.createElement("div");
  head.className = "code-head";
  const label = document.createElement("span");
  label.textContent = language || "code";
  const copy = document.createElement("button");
  copy.type = "button";
  copy.className = "copy-code";
  copy.textContent = "Sao chép";
  copy.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(code); copy.textContent = "Đã sao chép"; }
    catch (_) { showToast("Không thể truy cập clipboard"); }
    setTimeout(() => { copy.textContent = "Sao chép"; }, 1400);
  });
  const pre = document.createElement("pre");
  const codeElement = document.createElement("code");
  codeElement.textContent = code;
  pre.append(codeElement);
  head.append(label, copy);
  wrap.append(head, pre);
  parent.append(wrap);
}

function appendParagraphs(parent, text) {
  if (!text) return;
  const blocks = text.replace(/^\n+|\n+$/g, "").split(/\n\s*\n/);
  for (const block of blocks) {
    if (!block) continue;
    const paragraph = document.createElement("p");
    const lines = block.split("\n");
    lines.forEach((line, index) => {
      if (index) paragraph.append(document.createElement("br"));
      appendInline(paragraph, line);
    });
    parent.append(paragraph);
  }
}

function appendInline(parent, text) {
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*)/g;
  let last = 0;
  let match;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > last) parent.append(document.createTextNode(text.slice(last, match.index)));
    const token = match[0];
    let node;
    if (token.startsWith("`")) {
      node = document.createElement("code");
      node.textContent = token.slice(1, -1);
    } else if (token.startsWith("**")) {
      node = document.createElement("strong");
      node.textContent = token.slice(2, -2);
    } else {
      node = document.createElement("em");
      node.textContent = token.slice(1, -1);
    }
    parent.append(node);
    last = pattern.lastIndex;
  }
  if (last < text.length) parent.append(document.createTextNode(text.slice(last)));
}

function setStreaming(value) {
  state.streaming = value;
  $("#send-button").disabled = value;
  $("#stop-button").hidden = !value;
  messageInput.disabled = value;
  providerSelect.disabled = value;
  modelInput.disabled = value;
  folderInput.disabled = value;
  permissionSelect.disabled = value;
  $("#mode-chip").disabled = value;
  $("#new-chat").disabled = value;
  renderChatList();
}

async function sendMessage() {
  const text = messageInput.value.trim();
  if (!text || state.streaming) return;
  if (!state.chatId) newChat();
  if (!currentProvider()) {
    showToast("Hãy thêm hoặc chọn một nguồn AI trong Cài đặt.");
    return;
  }
  if (!folderInput.value.trim()) {
    showToast("Hãy nhập thư mục làm việc.");
    return;
  }
  const chatId = state.chatId;
  state.messages.push({ role: "user", content: text });
  const assistant = { role: "assistant", content: "", reasoning: "", logs: [] };
  state.messages.push(assistant);
  const existing = [...state.messages].reverse().find((message) => message.role === "user");
  const memory = state.memoryChats.get(chatId) || { id: chatId };
  if (!memory.title || memory.title === "Chat mới") memory.title = existing.content.slice(0, 72) || "Chat mới";
  memory.updated_at = Date.now() / 1000;
  state.memoryChats.set(chatId, memory);
  renderMessages();
  renderChatList();
  messageInput.value = "";
  resizeComposer();
  setStreaming(true);
  try {
    const response = await api("/api/chat", {
      method: "POST",
      body: JSON.stringify({
        chat_id: chatId,
        provider_id: providerSelect.value,
        model: modelInput.value.trim(),
        folder: folderInput.value.trim(),
        permission: permissionSelect.value || "write",
        effort: state.effort,
        messages: state.messages.filter((message) => message !== assistant).map(({ role, content }) => ({ role, content })),
      }),
    });
    if (!response.body) throw new Error("Trình duyệt không hỗ trợ đọc luồng phản hồi");
    await readSse(response.body, (eventName, data) => {
      if (eventName === "delta") assistant.content += data.text || "";
      else if (eventName === "reasoning") assistant.reasoning += data.text || "";
      else if (eventName === "log") assistant.logs.push(data.text || "");
      else if (eventName === "error") assistant.error = data.text || "Đã xảy ra lỗi";
      if (["delta", "reasoning", "log", "error"].includes(eventName)) renderMessages();
    });
  } catch (error) {
    assistant.error = error.message;
    renderMessages();
    showToast(error.message);
  } finally {
    setStreaming(false);
    memory.updated_at = Date.now() / 1000;
    await refreshChats();
    messageInput.focus();
  }
}

async function readSse(stream, onEvent) {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let boundary;
    while ((boundary = buffer.search(/\r?\n\r?\n/)) !== -1) {
      const block = buffer.slice(0, boundary);
      const match = buffer.slice(boundary).match(/^\r?\n\r?\n/);
      buffer = buffer.slice(boundary + (match ? match[0].length : 2));
      parseSseBlock(block, onEvent);
    }
  }
  buffer += decoder.decode();
  if (buffer.trim()) parseSseBlock(buffer, onEvent);
}

function parseSseBlock(block, onEvent) {
  let eventName = "message";
  const dataLines = [];
  for (const line of block.split(/\r?\n/)) {
    if (line.startsWith(":")) continue;
    if (line.startsWith("event:")) eventName = line.slice(6).trim();
    if (line.startsWith("data:")) dataLines.push(line.slice(5).trimStart());
  }
  if (!dataLines.length) return;
  try { onEvent(eventName, JSON.parse(dataLines.join("\n"))); }
  catch (_) { onEvent("error", { text: "Dữ liệu phản hồi không hợp lệ" }); }
}

async function stopChat() {
  if (!state.chatId || !state.streaming) return;
  $("#stop-button").disabled = true;
  try {
    const result = await api(`/api/chat/${encodeURIComponent(state.chatId)}/stop`, { method: "POST" });
    if (!result.stopped) showToast("Chat này không còn chạy.");
  } catch (error) { showToast(error.message); }
  finally { $("#stop-button").disabled = false; }
}

function resizeComposer() {
  messageInput.style.height = "auto";
  messageInput.style.height = `${Math.min(messageInput.scrollHeight, 180)}px`;
}

async function loadConfig() {
  state.config = await api("/api/config");
  setupProviderControls(state.config.providers[0]?.id);
  folderInput.value = state.config.default_folder || "";
  renderPlannerSettings();
}

function el(tag, props = {}, children = []) {
  const node = document.createElement(tag);
  const { dataset, ...rest } = props;
  Object.assign(node, rest);
  if (dataset) Object.assign(node.dataset, dataset);
  node.append(...children);
  return node;
}

function renderPlannerSettings() {
  const select = $("#planner-provider");
  if (!select) return;
  select.replaceChildren(...(state.config?.providers || []).map(provider =>
    el("option", { value: provider.id, textContent: provider.name })));
  const preferred = state.config?.planner_provider_id || ["claude_cli", "anthropic", "codex_cli"]
    .map(kind => state.config?.providers.find(provider => provider.kind === kind))
    .find(provider => provider && (state.providerStatuses.get(provider.id)?.ok ?? true))?.id
    || state.config?.providers[0]?.id;
  select.value = preferred || "";
  const provider = state.config?.providers.find(item => item.id === select.value);
  $("#planner-model").value = state.config?.planner_model || provider?.models[0] || "";
  $("#planner-model-options").replaceChildren(...(provider?.models || []).map(model => el("option", { value: model })));
}

async function savePlannerConfig() {
  state.config.planner_provider_id = $("#planner-provider").value;
  state.config.planner_model = $("#planner-model").value.trim();
  await api("/api/config", { method: "PUT", body: JSON.stringify({
    default_folder: state.config.default_folder,
    planner_provider_id: state.config.planner_provider_id,
    planner_model: state.config.planner_model,
  }) });
}

async function loadPlans() {
  state.plans = await api("/api/plans");
  renderPlanList();
}

function renderPlanList() {
  const root = $("#plans-list");
  root.replaceChildren();
  if (!state.plans.length) { root.textContent = "Chưa có kế hoạch."; return; }
  for (const plan of state.plans) {
    root.append(el("button", { className: "plan-list-item",
      textContent: `${plan.title} · ${plan.tasks.length} công việc${plan.approved ? " · đã duyệt" : " · bản nháp"}`,
      onclick: () => openPlan(plan) }));
  }
}

async function planFromRequest(text) {
  const request = text.trim() || [...state.messages].reverse().find(message => message.role === "user")?.content || "";
  if (!request) { showToast("Nhập yêu cầu hoặc mở chat có yêu cầu trước."); return; }
  const buttons = [$("#plan-button"), $("#hint-plan")];
  buttons.forEach(button => { if (button) button.disabled = true; });
  $(".main-panel").hidden = true;
  $("#extensions-view").hidden = true;
  $("#plans-view").hidden = false;
  const provider = state.config.providers.find(item => item.id === $("#planner-provider").value);
  const root = $("#plans-list");
  root.replaceChildren();
  const card = el("section", { className: "plan-progress" });
  const label = el("p", { textContent: `Đang lập kế hoạch bằng ${provider?.name || "provider"} · 00:00` });
  const spinner = el("span", { className: "spinner", ariaLabel: "Đang lập kế hoạch" });
  const logs = el("pre", { className: "task-log" });
  const cancel = el("button", { className: "subtle-button", textContent: "Hu?" });
  const controller = new AbortController();
  cancel.onclick = () => controller.abort();
  card.append(label, spinner, logs, cancel);
  root.append(card);
  const started = Date.now();
  const timer = setInterval(() => { const seconds = Math.floor((Date.now() - started) / 1000); label.textContent = `Đang lập kế hoạch bằng ${provider?.name || "provider"} · ${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`; }, 1000);
  try {
    const response = await fetch("/api/plans", { method: "POST", signal: controller.signal,
      headers: { "Content-Type": "application/json" }, body: JSON.stringify({ request, folder: folderInput.value.trim(), provider_id: $("#planner-provider").value || state.config.planner_provider_id, model: $("#planner-model").value || state.config.planner_model }) });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    if (!response.body) throw new Error("Browser does not support response streaming");
    let receivedPlan = null;
    await readSse(response.body, (event, data) => {
      if (event === "log") logs.textContent += `${data.text || ""}\n`;
      if (event === "error") throw new Error(data.message || data.text || "Planning failed");
      if (event === "plan") receivedPlan = data;
    });
    if (receivedPlan) { await loadPlans(); openPlan(receivedPlan); }
  } catch (error) {
    if (error.name === "AbortError") {
      logs.textContent += "\u0110\u00e3 hu\u1ef7\n";
      label.textContent = "\u0110\u00e3 hu\u1ef7";
    }
    else { logs.textContent += `${error.message}\n`; }
  } finally { clearInterval(timer); buttons.forEach(button => { if (button) button.disabled = false; }); }
}

function openPlan(plan) {
  state.currentPlan = structuredClone(plan);
  $(".main-panel").hidden = true;
  $("#extensions-view").hidden = true;
  $("#plans-view").hidden = false;
  renderPlanEditor();
}

function renderPlanEditor() {
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
    editor.append(el("input", { value: task.title, placeholder: "Tên việc", oninput: event => task.title = event.target.value }), el("textarea", { value: task.detail, placeholder: "Tệp cần sửa và kết quả mong đợi", oninput: event => task.detail = event.target.value })); cell(editor);
    const assignee = el("select");
    for (const provider of state.config.providers) assignee.append(el("option", { value: provider.id, textContent: provider.name }));
    assignee.value = task.assignee;
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
    const provider = state.config.providers.find(item => item.id === task.assignee);
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
    const kind = el("select"); for (const value of ["plan", "code", "test", "docs", "review"]) kind.append(el("option", { value, textContent: value })); kind.value = task.kind; kind.onchange = () => task.kind = kind.value; cell(kind);
    const status = el("span", { textContent: `${task.status === "waiting" ? "chờ" : task.status}${task.tokens ? ` · ${task.tokens} tokens` : ""}${task.log?.length ? ` · ${task.log.at(-1)}` : ""}`, dataset: { taskStatus: String(task.id) } }); cell(status);
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
        disabled: isRunning || !depsDone,
        onclick: () => runPlanTask(task),
      }));
      if (isRunning) {
        actions.append(el("button", {
          textContent: "Dừng",
          onclick: async () => {
            task._stopped = true;
            if (task.chat_id) await api(`/api/chat/${encodeURIComponent(task.chat_id)}/stop`, { method: "POST" });
          },
        }));
      }
      if (task.status !== "xong" && !isRunning) {
        actions.append(el("button", {
          textContent: "Đánh dấu xong",
          onclick: async () => { task.status = "xong"; await persistPlan(); renderPlanEditor(); },
        }));
      }
    }
    cell(actions); body.append(row);
  }
  table.append(body); root.append(table);
  const approve = el("button", { textContent: plan.approved ? "Đã duyệt" : "Duyệt kế hoạch", disabled: plan.approved || hasCycle(plan.tasks), onclick: async () => { plan.approved = true; await persistPlan(); renderPlanEditor(); } });
  const add = el("button", {
    textContent: "+ Thêm công việc",
    onclick: () => {
      plan.tasks.push({
        id: Math.max(0, ...plan.tasks.map(task => task.id)) + 1,
        title: "Công việc mới", detail: "", assignee: state.config.providers[0]?.id || "",
        model: "", effort: "default", depends_on: [], kind: "code", status: "ch?",
        summary: "", log: [], tokens: 0,
      });
      renderPlanEditor();
    },
  });
  const regen = el("button", { textContent: "Lập lại kế hoạch", onclick: () => planFromRequest(plan.request) });
  root.append(el("p", { className: "token-usage", textContent: `Tổng token kế hoạch: ${plan.tasks.reduce((sum, task) => sum + (task.tokens || 0), 0)}` }), add, regen, approve);
  for (const task of plan.tasks) if (task.log?.length) root.append(el("pre", { className: "task-log", textContent: task.log.join("\n") }));
}

function hasCycle(tasks) {
  const byId = new Map(tasks.map(task => [task.id, task]));
  const visiting = new Set(), done = new Set();
  const visit = id => { if (done.has(id)) return false; if (visiting.has(id)) return true; visiting.add(id); for (const dep of byId.get(id)?.depends_on || []) if (byId.has(dep) && visit(dep)) return true; visiting.delete(id); done.add(id); return false; };
  return tasks.some(task => visit(task.id));
}

async function persistPlan() {
  // Keep the local plan object as the source of truth: running tasks hold references into it,
  // so replacing it with the server copy would make their later status updates get lost.
  const plan = state.currentPlan;
  const body = JSON.stringify(plan, (key, value) => (key.startsWith("_") ? undefined : value));
  await api(`/api/plans/${plan.id}`, { method: "PUT", body });
  await loadPlans();
}

async function runPlanTask(task) {
  task._stopped = false; task.summary = "";
  const plan = state.currentPlan, provider = state.config.providers.find(item => item.id === task.assignee);
  if (!provider) return;
  const prompt = `Kế hoạch: ${plan.title}
Công việc: ${task.title}
Chi tiết: ${task.detail}
Tóm tắt công việc phụ thuộc đã hoàn tất:
${task.depends_on.map(id => plan.tasks.find(item => item.id === id)?.summary || "").join("\n")}`;
  const chatId = task.chat_id || crypto.randomUUID(); task.chat_id = chatId; task.status = "đang chạy"; task.log = [];
  await persistPlan(); renderPlanEditor();
  let hadError = false;
  try {
    const response = await api("/api/chat", {
      method: "POST",
      body: JSON.stringify({
        chat_id: chatId, provider_id: provider.id,
        model: task.model || provider.models[0] || "", effort: task.effort || "default",
        folder: plan.folder,
        permission: ["code", "test", "docs"].includes(task.kind) ? "write" : "read",
        messages: [{ role: "user", content: prompt }],
      }),
    });
    await readSse(response.body, (event, data) => {
      if (event === "error") { hadError = true; task.log.push(data.text || "Lỗi"); }
      if (event === "delta") task.summary += data.text || "";
      if (event === "log") {
        const text = data.text || "";
        task.log.push(text);
        const usage = text.match(/^Token: vào (\d+).*?ra (\d+)/);
        if (usage) task.tokens = (task.tokens || 0) + Number(usage[1]) + Number(usage[2]);
      }
      const node = $(`[data-task-status="${task.id}"]`);
      if (node) node.textContent = `${task.status} · ${task.log.at(-1) || ""}`;
    });
    task.status = task._stopped ? "đã dừng" : hadError ? "lỗi" : "xong";
  } catch (error) { hadError = true; task.status = task._stopped ? "đã dừng" : "lỗi"; task.log.push(error.message); }
  await persistPlan(); renderPlanEditor();
}
function clearProviderForm() {
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

function renderProviderSettingsList() {
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

function editProvider(id) {
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

function updateConnectionLine() {
  const id = state.settingsProviderId;
  const status = state.providerStatuses.get(id);
  const line = $("#provider-connection");
  const busy = state.providerChecking.has(id);
  line.className = `connection-detail ${busy ? "checking" : (status?.status || "unknown")}`;
  line.textContent = busy ? "Đang kiểm tra…" : (status?.message || "Chưa kiểm tra");
  line.title = status?.detail || "";
  $("#provider-test").disabled = !id || busy;
}

async function testProvider(id, toastResult = true) {
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

async function testAllProviders() { await Promise.all((state.config?.providers || []).map((p) => testProvider(p.id, true))); }

function updateProviderFormKind() {
  const kind = $("#provider-kind").value;
  const showUrl = kind === "openai_compat";
  const showExecutable = kind === "codex_cli" || kind === "claude_cli";
  $("#base-url-field").hidden = !showUrl;
  $("#provider-executable-field").hidden = !showExecutable;
  $("#cli-discover").hidden = !showExecutable;
  $("#models-load").hidden = !showUrl;
}

async function saveProvider(event, testAfterSave = true) {
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

async function deleteProvider() {
  if (!state.settingsProviderId || !window.confirm("Xóa nguồn AI này?")) return;
  try {
    state.config = await api(`/api/providers/${encodeURIComponent(state.settingsProviderId)}`, { method: "DELETE" });
    clearProviderForm();
    setupProviderControls(state.config.providers[0]?.id);
  } catch (error) { $("#provider-form-note").textContent = error.message; }
}

async function loadModels() {
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

async function refreshCliStatus() {
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

async function discoverCli() {
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
          showToast(`? ?? t?m th?y v? k?t n?i ${cliName} (${candidate.version})`);
        } else {
          showToast(`? ${cliName}: ${status?.message || "Kh?ng k?t n?i ???c"}`);
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

async function saveDefaultFolder() {
  if (!state.config || !folderInput.value.trim()) return;
  state.config.default_folder = folderInput.value.trim();
  try { state.config = await api("/api/config", { method: "PUT", body: JSON.stringify({ default_folder: state.config.default_folder, planner_provider_id: state.config.planner_provider_id || "", planner_model: state.config.planner_model || "" }) }); }
  catch (error) { showToast(error.message); }
}

async function loadExtensions() {
  const folder=folderInput.value.trim(); const cards=await api(`/api/extensions${folder?`?folder=${encodeURIComponent(folder)}`:""}`);
  const root=$("#extension-cards"); root.replaceChildren();
  for(const card of cards){const article=document.createElement("article");article.className="extension-card";
    const title=document.createElement("h2");title.textContent=card.name;const desc=document.createElement("p");desc.textContent=card.description;
    const link=document.createElement("a");link.href=card.homepage;link.target="_blank";link.rel="noopener";link.textContent="Trang chủ ↗";
    const badges=document.createElement("div");badges.className="extension-badges";const badge=(s)=>{const x=document.createElement("span");x.className="extension-badge";x.textContent=s;badges.append(x)};
    badge(card.installed?`Đã cài · ${card.version||"đã cài"}`:"Chưa cài");badge(`Codex: ${card.codex?"đã kết nối":"chưa"}`);badge(`Claude: ${card.claude?"đã kết nối":"chưa"}`);if(card.folder_indexed!==null)badge(`Thư mục hiện tại: ${card.folder_indexed?"đã lập chỉ mục":"chưa"}`);
    const actions=document.createElement("div");actions.className="extension-actions";
    for(const action of card.actions){if((action.id==="install"&&card.installed)||(action.id==="upgrade"&&!card.installed)||(action.id.startsWith("connect_")&&!card.installed)||(action.id==="index_folder"&&(!card.installed||!folder))||(action.id==="uninstall"&&!card.installed))continue;
      const button=document.createElement("button");button.className=action.id==="uninstall"?"danger-button":"subtle-button";button.textContent=action.label;button.addEventListener("click",()=>runExtensionAction(card,action));actions.append(button)}
    article.append(title,desc,link,badges,actions);root.append(article);
  }
}
async function runExtensionAction(card,action){
  const command=`${action.command}${action.id==="index_folder"?`  (cwd: ${folderInput.value.trim()})`:""}`;
  if(!window.confirm(`Thực hiện: ${action.label}\n\nLệnh: ${command}\n\nCó thể sửa: ${action.modifies}\n\nTiếp tục?`))return;
  const log=$("#extension-log");log.textContent+=`\n> ${command}\n`;$("#extension-stop").hidden=false;
  try{const response=await fetch(`/api/extensions/${card.id}/actions/${action.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({folder:folderInput.value.trim()})});if(!response.ok)throw new Error((await response.json()).error||`HTTP ${response.status}`);
    await readSse(response.body,(event,data)=>{if(event==="log")log.textContent+=`${data}\n`;if(event==="error"){showToast(`✗ ${data.message||"Lệnh thất bại"}`);log.textContent+=`${data.message||"Lệnh thất bại"}\n`}if(event==="done"&&data.ok)showToast("✓ Hoàn tất")});
  }catch(error){showToast(`✗ ${error.message}`);log.textContent+=`${error.message}\n`}finally{$("#extension-stop").hidden=true;await loadExtensions()}
}
$("#extensions-open").addEventListener("click",async()=>{$(".main-panel").hidden=true;$("#extensions-view").hidden=false;try{await loadExtensions()}catch(e){showToast(e.message)}});
$("#extensions-back").addEventListener("click",()=>{$("#extensions-view").hidden=true;$(".main-panel").hidden=false});
$("#plans-open").addEventListener("click",async()=>{$(".main-panel").hidden=true;$("#extensions-view").hidden=true;$("#plans-view").hidden=false;$("#plan-editor").hidden=true;try{await loadPlans()}catch(e){showToast(e.message)}});
$("#plans-back").addEventListener("click",()=>{$("#plans-view").hidden=true;$(".main-panel").hidden=false});
$("#plan-button").addEventListener("click",()=>planFromRequest(messageInput.value));
$("#hint-plan").addEventListener("click",()=>planFromRequest(messageInput.value));
$("#hint-dismiss").addEventListener("click",()=>{$("#plan-hint").hidden=true});
messageInput.addEventListener("input",()=>{const v=messageInput.value.toLowerCase();const suggest=v.length>300||["làm app","xây dựng","thêm tính năng","refactor"].some(x=>v.includes(x));$("#plan-hint").hidden=!suggest});
$("#planner-provider").addEventListener("change",()=>{const p=state.config.providers.find(x=>x.id===$("#planner-provider").value);$("#planner-model").value=p?.models[0]||"";renderPlannerSettings();savePlannerConfig().catch(e=>showToast(e.message))});
$("#planner-model").addEventListener("change",()=>savePlannerConfig().catch(e=>showToast(e.message)));
$("#extension-stop").addEventListener("click",()=>fetch("/api/extensions/codegraph/stop",{method:"POST"}));

$("#new-chat").addEventListener("click", newChat);
$("#settings-open").addEventListener("click", async () => {
  $("#settings-dialog").showModal();
  renderPlannerSettings();
  if (state.settingsProviderId) editProvider(state.settingsProviderId);
  else clearProviderForm();
  testAllProviders();
  await refreshCliStatus();
});
$("#provider-select").addEventListener("change", () => { updateModels(); updatePermissionControl(); rememberCurrentChat(); saveLastMode(); });
$("#model-input").addEventListener("input", () => { renderModePicker(); rememberCurrentChat(); saveLastMode(); });
$("#mode-chip").addEventListener("click", () => {
  const pop = $("#mode-popover"); pop.hidden = !pop.hidden;
  $("#mode-chip").setAttribute("aria-expanded", String(!pop.hidden));
  if (!pop.hidden) pop.querySelector("button:not([disabled])")?.focus();
});
$("#mode-popover").addEventListener("click", (event) => {
  const providerId = event.target.closest("[data-provider]")?.dataset.provider;
  const model = event.target.closest("[data-model]")?.dataset.model;
  const effort = event.target.closest("[data-effort]")?.dataset.effort;
  if (providerId) { providerSelect.value = providerId; updateModels(); updatePermissionControl(); }
  if (model) modelInput.value = model;
  if (effort) state.effort = effort;
  if (providerId || model || effort) { renderModePicker(); rememberCurrentChat(); saveLastMode(); }
  if (providerId || model) { $("#mode-popover").hidden = true; $("#mode-chip").setAttribute("aria-expanded", "false"); $("#mode-chip").focus(); }
});
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
document.addEventListener("click", (event) => { const pop = $("#mode-popover"); if (!pop.hidden && !event.target.closest(".mode-picker")) { pop.hidden = true; $("#mode-chip").setAttribute("aria-expanded", "false"); } });
$("#composer").addEventListener("submit", (event) => { event.preventDefault(); sendMessage(); });
$("#stop-button").addEventListener("click", stopChat);
$("#message-input").addEventListener("input", resizeComposer);
$("#message-input").addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    sendMessage();
  }
});
$("#folder-input").addEventListener("input", rememberCurrentChat);
permissionSelect.addEventListener("change", rememberCurrentChat);
$("#folder-input").addEventListener("change", saveDefaultFolder);
$("#provider-add").addEventListener("click", clearProviderForm);
$("#provider-kind").addEventListener("change", updateProviderFormKind);
$("#provider-form").addEventListener("submit", saveProvider);
$("#provider-delete").addEventListener("click", deleteProvider);
$("#provider-test").addEventListener("click", () => testProvider(state.settingsProviderId));
$("#cli-discover").addEventListener("click", discoverCli);
$("#models-load").addEventListener("click", loadModels);
$("#cli-refresh").addEventListener("click", refreshCliStatus);

(async function init() {
  try {
    await loadConfig();
    renderModePicker();
    await refreshChats();
    newChat();
  } catch (error) {
    showToast(error.message);
  }
})();
