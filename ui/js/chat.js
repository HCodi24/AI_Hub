import { hideMemory, memoryEvent } from "./memory.js";
import { $, showToast } from "./util.js";
import { state } from "./state.js";
import { hideDevPage, refreshDevRepo } from "./dev.js";
import { EFFORTS, currentProvider, renderModePicker, saveLastMode, setupProviderControls, updateModels, updatePermissionControl } from "./mode-chip.js";
import { api, readSse } from "./api.js";
import { resolveConnectedRole, testAllProviders } from "./settings.js";
import { planFromRequest } from "./plans.js";
import { setDebateMarkdown } from "./debate-card.js";

export const providerSelect = $("#provider-select");
export const modelInput = $("#model-input");
export const modelOptions = $("#model-options");
export const folderInput = $("#folder-input");
export const permissionSelect = $("#permission-select");
export const fileToolsSelect = $("#file-tools-select");
export const messagesRoot = $("#messages");
export const welcome = $("#welcome");
export const messageInput = $("#message-input");
const attachmentInput = $("#attachment-input");
const attachmentTray = $("#composer-attachments");
let pendingAttachments = [];
const imageExtensions = { "image/png": "png", "image/jpeg": "jpg", "image/webp": "webp", "image/gif": "gif" };

export async function addImageFiles(files) {
  const selected = [...files];
  if (!selected.length) return;
  if (!state.chatId) newChat();
  for (const file of selected) {
    if (pendingAttachments.length >= 5) { showToast("Mỗi tin nhắn tối đa 5 ảnh"); break; }
    if (file.size > 10 * 1024 * 1024) { showToast(`${file.name}: ảnh vượt quá 10 MB`); continue; }
    if (file.type && !Object.hasOwn(imageExtensions, file.type)) { showToast(`${file.name}: chỉ hỗ trợ PNG, JPEG, WebP và GIF`); continue; }
    try {
      const dataUrl = await readFileDataUrl(file);
      const result = await api("/api/attachments", { method: "POST", body: JSON.stringify({ chat_id: state.chatId, data: dataUrl.split(",")[1] }) });
      pendingAttachments.push({ ...result, preview: URL.createObjectURL(file) });
      renderAttachmentTray();
    } catch (error) { showToast(error.message); }
  }
  attachmentInput.value = "";
}

function readFileDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("Không thể đọc ảnh"));
    reader.readAsDataURL(file);
  });
}

function renderAttachmentTray() {
  attachmentTray.replaceChildren();
  for (const [index, attachment] of pendingAttachments.entries()) {
    const thumb = document.createElement("div"); thumb.className = "attachment-thumb";
    const image = document.createElement("img"); image.src = attachment.preview; image.alt = "Ảnh đính kèm";
    const remove = document.createElement("button"); remove.type = "button"; remove.className = "attachment-remove"; remove.textContent = "×"; remove.setAttribute("aria-label", "Xóa ảnh đính kèm");
    remove.addEventListener("click", () => { URL.revokeObjectURL(attachment.preview); pendingAttachments.splice(index, 1); renderAttachmentTray(); });
    thumb.append(image, remove); attachmentTray.append(thumb);
  }
}

function imageUrl(chatId, attachment) {
  const extension = imageExtensions[attachment.mime];
  return extension ? `/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(attachment.id)}.${extension}` : "";
}
export function newChat() {
  if (state.streaming) return;
  clearPendingAttachments();
  state.chatId = crypto.randomUUID();
  state.devChat = false;
  state.messages = [];
  state.memoryChats.set(state.chatId, { id: state.chatId, title: "Chat mới", updated_at: Date.now() / 1000 });
  let lastMode = null;
  try { lastMode = JSON.parse(localStorage.getItem("ai-hub-last-mode") || "null"); } catch (_) { /* Ignore invalid or unavailable storage. */ }
  if (!state.config?.providers.some((provider) => provider.id === lastMode?.provider_id)) lastMode = null;
  setupProviderControls(lastMode?.provider_id || providerSelect.value || state.config?.providers[0]?.id, lastMode?.model || "");
  state.selectedSource = lastMode?.source === "mutis" ? "mutis" : "provider";
  messageInput.placeholder = state.selectedSource === "mutis" ? "Nhắn tin với MutiS — bấm ✦ Gợi ý chia việc để chia việc cho 3 AI…" : "Nhắn tin với AI…";
  state.effort = EFFORTS.includes(lastMode?.effort) ? lastMode.effort : "default";
  renderModePicker();
  permissionSelect.value = "write";
  fileToolsSelect.value = "none";
  folderInput.value = state.config?.default_folder || "";
  rememberCurrentChat();
  renderMessages();
  renderChatList();
  renderDevChatControls();
  messageInput.focus();
}
export async function loadChat(id) {
  if (state.streaming) return;
  clearPendingAttachments();
  const saved = state.chats.some((chat) => chat.id === id);
  const cached = state.memoryData.get(id);
  if (!saved && cached) {
    state.chatId = id;
    state.devChat = Boolean(cached.dev_chat);
    state.messages = cached.messages.map((message) => ({ ...message, logs: [...(message.logs || [])] }));
    let mode = null;
    try { mode = JSON.parse(localStorage.getItem(`ai-hub-last-mode-${id}`) || "null"); } catch (_) { /* Ignore invalid mode cache. */ }
    state.selectedSource = mode?.source === "mutis" ? "mutis" : "provider";
    messageInput.placeholder = state.selectedSource === "mutis" ? "Nhắn tin với MutiS — bấm ✦ Gợi ý chia việc để chia việc cho 3 AI…" : "Nhắn tin với AI…";
    setupProviderControls(cached.provider_id, cached.model);
    permissionSelect.value = cached.permission || "write";
    fileToolsSelect.value = cached.file_tools || "none";
    state.effort = cached.effort || "default";
    renderModePicker();
    folderInput.value = cached.folder || state.config?.default_folder || "";
    renderMessages();
    renderChatList();
    renderDevChatControls();
    return;
  }
  try {
    const chat = await api(`/api/chats/${encodeURIComponent(id)}`);
    state.chatId = chat.id;
    state.devChat = Boolean(chat.dev_chat);
    state.messages = chat.messages || [];
    let mode = null;
    try { mode = JSON.parse(localStorage.getItem(`ai-hub-last-mode-${id}`) || "null"); } catch (_) { /* Ignore invalid mode cache. */ }
    state.selectedSource = mode?.source === "mutis" ? "mutis" : "provider";
    messageInput.placeholder = state.selectedSource === "mutis" ? "Nhắn tin với MutiS — bấm ✦ Gợi ý chia việc để chia việc cho 3 AI…" : "Nhắn tin với AI…";
    state.memoryChats.set(chat.id, { id: chat.id, title: chat.title, updated_at: chat.updated_at, dev_chat: chat.dev_chat });
    setupProviderControls(chat.provider_id, chat.model);
    permissionSelect.value = chat.permission || "write";
    fileToolsSelect.value = chat.file_tools || "none";
    state.effort = chat.effort || "default";
    renderModePicker();
    folderInput.value = chat.folder || state.config?.default_folder || "";
    renderMessages();
    renderChatList();
    renderDevChatControls();
  } catch (error) {
    showToast(error.message);
  }
}
function clearPendingAttachments() {
  for (const attachment of pendingAttachments) URL.revokeObjectURL(attachment.preview);
  pendingAttachments = [];
  renderAttachmentTray();
}
export async function refreshChats() {
  try {
    state.chats = await api("/api/chats");
    renderChatList();
  } catch (error) {
    showToast(error.message);
  }
}
export function renderChatList() {
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
    select.textContent = `${chat.dev_chat ? "⚒ " : ""}${chat.title || "Chat mới"}`;
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
export function renderDevChatControls() {
  const strip = $("#dev-chat-strip");
  strip.hidden = !state.devChat;
  folderInput.readOnly = state.devChat;
  messageInput.placeholder = state.devChat ? "Mô tả thay đổi cho AI Hub…" : state.selectedSource === "mutis" ? "Nhắn tin với MutiS — bấm ✦ Gợi ý chia việc để chia việc cho 3 AI…" : "Nhắn tin với AI…";
  if (state.devChat) strip.textContent = `⚒ Chat tự phát triển · thư mục: ${folderInput.value}${state.devChat && !state.dev ? " (chế độ dev đang tắt)" : ""}`;
}
export function devChatPreamble(repo) {
  return [
    `Bạn đang sửa chính ứng dụng AI Hub (repo: ${repo}) đang chạy ở chế độ dev.\n`,
    `- Giao diện ở ui/ tự tải lại khi lưu; code Rust ở src/ cần người dùng bấm 'Build & khởi động lại'.\n`,
    `- Không chạy \`cargo build --release\`, không tắt tiến trình ai-hub, không sửa target/.\n`,
    `- Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy \`cargo test\` với CARGO_TARGET_DIR=target\\dev-check nếu cần.\n`,
    `- Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không.`,
  ].join("");
}
export async function openDevChat() {
  if (!state.devRepo) await refreshDevRepo();
  if (!state.devRepo?.path) { showToast("Không lấy được đường dẫn repo"); return; }
  hideDevPage(); newChat(); state.devChat = true;
  folderInput.value = state.devRepo.path;
  const mutis = state.mutis?.enabled;
  const provider = state.config.providers.find(item => item.kind === "codex_cli");
  if (mutis) state.selectedSource = "mutis";
  else if (provider) { providerSelect.value = provider.id; state.selectedSource = "provider"; updateModels(); updatePermissionControl(); state.effort = "high"; }
  permissionSelect.value = "write";
  messageInput.placeholder = "Mô tả thay đổi cho AI Hub…";
  messageInput.value = "";
  fileToolsSelect.value = "none";
  messageInput.focus(); rememberCurrentChat(); renderModePicker(); renderDevChatControls(); renderChatList();
  state.memoryChats.get(state.chatId).dev_chat = true;
}
export function renderMessages() {
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
      if (message.diff) appendFoldout(body, "Thay đổi", message.diff);
      if (message.logs?.length) appendActivityFoldout(body, message.logs, tokenLog);
      if (message.error) {
        const error = document.createElement("div");
        error.className = "error-text";
        error.textContent = message.error;
        body.append(error);
      }
    }
    if (message.role === "user" && message.attachments?.length) {
      const thumbs = document.createElement("div"); thumbs.className = "message-attachments";
      for (const attachment of message.attachments) {
        const url = imageUrl(state.chatId, attachment);
        if (!url) continue;
        const link = document.createElement("a"); link.href = url; link.target = "_blank"; link.rel = "noopener"; link.title = "Mở ảnh kích thước đầy đủ";
        const image = document.createElement("img"); image.src = url; image.alt = "Ảnh đã gửi"; link.append(image); thumbs.append(link);
      }
      body.append(thumbs);
    }
    renderMarkdown(body, message.role === "assistant" ? hideMemory(message.content || "") : message.content || "");
    if (tokenLog) {
      const usage = document.createElement("div");
      usage.className = "token-usage";
      usage.textContent = tokenLog;
      body.append(usage);
    }
    if (message.running) {
      const live = document.createElement("div");
      live.className = "running-indicator";
      const dots = document.createElement("span"); dots.className = "running-dots"; dots.append(...[0, 1, 2].map(() => document.createElement("i")));
      const text = document.createElement("span"); text.textContent = `${message.running} đang chạy…`;
      live.append(dots, text);
      body.append(live);
    }
    row.append(body);
    messagesRoot.append(row);
  }
  messagesRoot.scrollTop = messagesRoot.scrollHeight;
}
export function rememberCurrentChat() {
  if (!state.chatId) return;
  state.memoryData.set(state.chatId, {
    provider_id: providerSelect.value,
    model: modelInput.value,
    folder: folderInput.value,
    permission: permissionSelect.value || "write",
    effort: state.effort,
    dev_chat: state.devChat,
    file_tools: fileToolsSelect.value || "none",
    messages: state.messages.map((message) => ({ ...message, logs: [...(message.logs || [])] })),
  });
}
export function appendFoldout(parent, title, text) {
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
export function appendActivityFoldout(parent, logs, tokenLog) {
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
export function renderMarkdown(parent, source) {
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
export function appendCodeBlock(parent, language, code) {
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
export function appendParagraphs(parent, text) {
  if (!text) return;
  const blocks = text.replace(/^\n+|\n+$/g, "").split(/\n\s*\n/);
  for (const block of blocks) {
    if (!block) continue;
    // Headings (#..######) and bullet / numbered lists; any other lines form a paragraph with line breaks.
    let paragraph = null;
    let list = null;
    for (const line of block.split("\n")) {
      const heading = line.match(/^(#{1,6})\s+(.*)$/);
      const item = line.match(/^\s*(?:([-*+])|(\d+)[.)])\s+(.*)$/);
      if (heading) {
        paragraph = null; list = null;
        const node = document.createElement(`h${Math.min(heading[1].length + 2, 6)}`);
        appendInline(node, heading[2]);
        parent.append(node);
      } else if (item) {
        paragraph = null;
        const tag = item[1] ? "ul" : "ol";
        if (!list || list.tagName.toLowerCase() !== tag) { list = document.createElement(tag); parent.append(list); }
        const li = document.createElement("li");
        appendInline(li, item[3]);
        list.append(li);
      } else {
        list = null;
        if (paragraph) paragraph.append(document.createElement("br"));
        else { paragraph = document.createElement("p"); parent.append(paragraph); }
        appendInline(paragraph, line);
      }
    }
  }
}
export function appendInline(parent, text) {
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
// Short "who is answering" text for the live status strip.
export function runningLabel(status = null) {
// File tools only apply to sources that allow agent mode; every other source always sends "none".
function activeFileTools() {
  return currentProvider()?.agent_capable ? fileToolsSelect.value || "none" : "none";
}

  if (status?.provider_name) {
    const effort = status.effort ? ` · ${status.effort}` : "";
    return `${status.provider_name} · ${status.model || "model"}${effort}`;
  }
  const provider = currentProvider();
  const name = provider ? provider.name : "AI";
  const model = modelInput.value ? ` · ${modelInput.value}` : "";
  return `${name}${model}`;
}

let liveStatus = null;
let statusTimer = null;
function renderRunningStatus() {
  const strip = $("#running-model");
  if (!strip) return;
  strip.replaceChildren();
  strip.hidden = !state.streaming;
  if (!state.streaming) return;
  const elapsed = liveStatus?.elapsed_s || 0;
  const mmss = `${String(Math.floor(elapsed / 60)).padStart(2, "0")}:${String(elapsed % 60).padStart(2, "0")}`;
  const dot = document.createElement("i"); dot.className = "running-dot";
  const identity = document.createElement("span"); identity.textContent = `${liveStatus?.role ? `${liveStatus.role} · ` : ""}${runningLabel(liveStatus)}`;
  const time = document.createElement("span"); time.className = "running-time"; time.textContent = mmss;
  strip.append(dot, identity, time);
  const idle = liveStatus?.idle_s || 0;
  if (liveStatus) {
    const activity = document.createElement("span"); activity.className = "running-activity";
    activity.textContent = `hoạt động cuối: ${liveStatus.last_activity || "đang chờ phản hồi"} (${idle}s trước)`;
    strip.append(activity);
  }
  strip.classList.toggle("warning", idle >= 45);
  strip.classList.toggle("critical", idle >= 120);
  if (idle >= 45) {
    const warning = document.createElement("span"); warning.className = "running-warning";
    warning.textContent = `⚠ Chưa có phản hồi 45s — AI có thể đang suy nghĩ lâu hoặc bị treo${idle >= 120 ? ". Bấm Dừng rồi gửi lại nếu cần" : ""}`;
    strip.append(warning);
    $("#stop-button").classList.add("stop-emphasis");
  } else $("#stop-button").classList.remove("stop-emphasis");
}

export function setStreaming(value) {
  state.streaming = value;
  $("#send-button").disabled = value;
  $("#attachment-button").disabled = value;
  $("#stop-button").hidden = !value;
  if (value) liveStatus = { provider_name: currentProvider()?.name || "AI", model: modelInput.value, effort: state.effort, idle_s: 0, elapsed_s: 0 };
  else { liveStatus = null; clearInterval(statusTimer); statusTimer = null; $("#stop-button").classList.remove("stop-emphasis"); }
  renderRunningStatus();
  messageInput.disabled = value;
  providerSelect.disabled = value;
  modelInput.disabled = value;
  folderInput.disabled = value;
  permissionSelect.disabled = value;
  $("#mode-chip").disabled = value;
  $("#new-chat").disabled = value;
  fileToolsSelect.disabled = value;
  renderChatList();
  if (value) {
    clearInterval(statusTimer);
    statusTimer = setInterval(() => { if (liveStatus) { liveStatus.elapsed_s += 1; liveStatus.idle_s += 1; renderRunningStatus(); } }, 1000);
  }
}
export async function sendMessage() {
  const text = messageInput.value.trim();
  if ((!text && !pendingAttachments.length) || state.streaming) return;
  if (!state.chatId) newChat();
  if (state.selectedSource === "mutis") {
    await testAllProviders();
    const lead = resolveConnectedRole("lead");
    if (!state.mutis?.enabled) {
      showToast("Hãy bật MutiS trong Cài đặt.");
      return;
    }
    if (!lead) {
      showToast("⚠ Không có nguồn nào đang kết nối cho vai trò lập kế hoạch & review. Hãy kết nối ít nhất một nguồn trong Cài đặt.");
      return;
    }
    if (lead.provider_id !== state.mutis.roles.lead.provider_id) {
      state.mutis.roles.lead = lead;
      state.mutis = await api("/api/mutis", { method: "PUT", body: JSON.stringify(state.mutis) });
    }
    // MutiS: a normal message is a NORMAL chat answered by the lead role. Planning / task splitting only happens
    // when the user explicitly clicks "✦ Gợi ý chia việc" (or the big-request hint).
    providerSelect.value = lead.provider_id;
    modelInput.value = lead.model || modelInput.value;
    if (lead.effort) state.effort = lead.effort;
    state.config.planner_provider_id = lead.provider_id;
    state.config.planner_model = lead.model;
    $("#planner-provider").value = lead.provider_id;
    $("#planner-model").value = lead.model;
    saveLastMode();
    // "Tự động chia việc": the message becomes a plan request right away (no normal chat answer).
    if (state.mutis.auto_plan && text && !state.devChat) {
      messageInput.value = "";
      planFromRequest(text);
      return;
    }
  }
  if (!currentProvider()) {
    showToast("Hãy thêm hoặc chọn một nguồn AI trong Cài đặt.");
    return;
  }
  if (!folderInput.value.trim()) {
    showToast("Hãy nhập thư mục làm việc.");
    return;
  }
  const chatId = state.chatId;
  const sentAttachments = pendingAttachments.map(({ id, mime }) => ({ id, mime }));
  state.messages.push({ role: "user", content: text, attachments: sentAttachments });
  clearPendingAttachments();
  const assistant = { role: "assistant", content: "", reasoning: "", logs: [], diff: "" };
  assistant.running = currentProvider()?.name || "AI";
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
        dev_chat: state.devChat,
        provider_id: providerSelect.value,
        model: modelInput.value.trim(),
        folder: folderInput.value.trim(),
        permission: permissionSelect.value || "write",
        effort: state.effort,
        messages: state.messages.filter((message) => message !== assistant).map(({ role, content, attachments }) => ({ role, content, attachments: attachments || [] })),
        file_tools: activeFileTools(),
      }),
    });
    if (!response.body) throw new Error("Trình duyệt không hỗ trợ đọc luồng phản hồi");
    let receivedDone = false;
    await readSse(response.body, (eventName, data) => {
      if (eventName === "status") {
        liveStatus = { ...liveStatus, ...data };
        assistant.running = [data.role, data.provider_name].filter(Boolean).join(" · ") || "AI";
        renderRunningStatus(); renderMessages();
      }
      else if (eventName === "heartbeat") { liveStatus = { ...liveStatus, ...data }; renderRunningStatus(); }
      else if (eventName === "done") { receivedDone = true; assistant.running = ""; }
      if (eventName === "delta") assistant.content += data.text || "";
      else if (eventName === "reasoning") assistant.reasoning += data.text || "";
      else if (eventName === "log") assistant.logs.push(data.text || "");
      else if (eventName === "diff") assistant.diff = data.text || "";
      else if (eventName === "error") assistant.error = data.text || "Đã xảy ra lỗi";
      if (["delta", "reasoning", "log", "diff", "error", "done"].includes(eventName)) renderMessages();
    });
    if (!receivedDone) throw new Error("Mất kết nối với app (có thể app đang khởi động lại). Tin nhắn có thể chưa hoàn tất — kiểm tra rồi gửi lại.");
  } catch (error) {
    assistant.running = "";
    assistant.error = "Mất kết nối với app (có thể app đang khởi động lại). Tin nhắn có thể chưa hoàn tất — kiểm tra rồi gửi lại.";
    renderMessages();
    showToast(error.message);
  } finally {
    setStreaming(false);
    memory.updated_at = Date.now() / 1000;
    await refreshChats();
    messageInput.focus();
  }
}
export function resizeComposer() {
  messageInput.style.height = "auto";
  messageInput.style.height = `${Math.min(messageInput.scrollHeight, 180)}px`;
}

export async function stopChat() {
  if (!state.chatId || !state.streaming) return;
  $("#stop-button").disabled = true;
  try {
    const result = await api(`/api/chat/${encodeURIComponent(state.chatId)}/stop`, { method: "POST" });
    if (!result.stopped) showToast("Chat này không còn chạy.");
  } catch (error) { showToast(error.message); }
  finally { $("#stop-button").disabled = false; }
}
      if (eventName === "memory") memoryEvent(data, folderInput.value);

setDebateMarkdown(renderMarkdown);
