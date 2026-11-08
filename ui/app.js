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
};

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
}

function newChat() {
  if (state.streaming) return;
  state.chatId = crypto.randomUUID();
  state.messages = [];
  state.memoryChats.set(state.chatId, { id: state.chatId, title: "Chat mới", updated_at: Date.now() / 1000 });
  setupProviderControls(providerSelect.value || state.config?.providers[0]?.id);
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
}

function clearProviderForm() {
  state.settingsProviderId = null;
  $("#provider-id").value = "";
  $("#provider-name").value = "";
  $("#provider-kind").value = "openai_compat";
  $("#provider-url").value = "";
  $("#provider-key").value = "";
  $("#provider-key").placeholder = "Nhập API key (không bắt buộc với CLI)";
  $("#provider-models").value = "";
  $("#provider-delete").hidden = true;
  $("#provider-form-note").textContent = "";
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
    button.textContent = provider.name;
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
  $("#provider-key").placeholder = provider.api_key ? "Key đã lưu — để trống để giữ nguyên" : "Nhập API key (không bắt buộc với CLI)";
  $("#provider-models").value = (provider.models || []).join("\n");
  $("#provider-delete").hidden = false;
  $("#provider-form-note").textContent = "";
  renderProviderSettingsList();
  updateProviderFormKind();
}

function updateProviderFormKind() {
  const showUrl = $("#provider-kind").value === "openai_compat";
  $("#base-url-field").hidden = !showUrl;
  $("#models-load").hidden = !showUrl;
}

async function saveProvider(event) {
  event.preventDefault();
  const id = state.settingsProviderId;
  const models = $("#provider-models").value.split(/\r?\n/).map((model) => model.trim()).filter(Boolean);
  const body = {
    name: $("#provider-name").value.trim(),
    kind: $("#provider-kind").value,
    base_url: $("#provider-url").value.trim(),
    api_key: $("#provider-key").value,
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
      detail.textContent = item.installed ? (item.version || "Đã cài, không đọc được phiên bản") : "Chưa tìm thấy trong PATH";
      card.append(title, detail);
      root.append(card);
    }
  } catch (error) { root.textContent = error.message; }
}

async function saveDefaultFolder() {
  if (!state.config || !folderInput.value.trim()) return;
  state.config.default_folder = folderInput.value.trim();
  try { state.config = await api("/api/config", { method: "PUT", body: JSON.stringify({ default_folder: state.config.default_folder }) }); }
  catch (error) { showToast(error.message); }
}

$("#new-chat").addEventListener("click", newChat);
$("#settings-open").addEventListener("click", async () => {
  $("#settings-dialog").showModal();
  if (state.settingsProviderId) editProvider(state.settingsProviderId);
  else clearProviderForm();
  await refreshCliStatus();
});
$("#provider-select").addEventListener("change", () => { updateModels(); updatePermissionControl(); rememberCurrentChat(); });
$("#model-input").addEventListener("input", rememberCurrentChat);
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
$("#models-load").addEventListener("click", loadModels);
$("#cli-refresh").addEventListener("click", refreshCliStatus);

(async function init() {
  try {
    await loadConfig();
    await refreshChats();
    newChat();
  } catch (error) {
    showToast(error.message);
  }
})();
