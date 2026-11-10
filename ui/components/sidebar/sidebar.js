// Thanh bên (sidebar): danh sách lịch sử chat và các nút điều hướng.
import { $, showToast } from "/js/util.js";
import { state } from "/js/state.js";
import { api } from "/js/api.js";
import { newChat, loadChat, openDevChat, refreshChats } from "/js/chat.js";
import { showDevPage } from "/js/dev.js";
import { loadMutisPage } from "/js/mutis-page.js";
import { loadPlans } from "/js/plans.js";
import { loadExtensions } from "/js/extensions.js";
import { clearProviderForm, editProvider, refreshCliStatus, renderPlannerSettings, testAllProviders } from "/js/settings.js";

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

// Hiện/ẩn các mục chỉ dành cho chế độ dev.
export function applyDevVisibility() {
  $("#dev-badge").hidden = !state.dev;
  $("#self-dev-open").hidden = !state.dev;
  $("#dev-chat-new").hidden = !state.dev;
}

export function initSidebar() {
  loadPlans().catch(error => showToast(error.message));
  $("#new-chat").addEventListener("click", newChat);
  $("#dev-chat-new").addEventListener("click", openDevChat);
  $("#self-dev-open").addEventListener("click", showDevPage);
  $("#mutis-open").addEventListener("click", async () => {
    $(".main-panel").hidden = true;
    $("#mutis-view").hidden = false;
    try {
      await loadMutisPage();
    } catch (e) {
      showToast(e.message);
    }
  });
  $("#settings-open").addEventListener("click", async () => {
    $("#settings-dialog").showModal();
    loadExtensions().catch(e => showToast(e.message));
    renderPlannerSettings();
    if (state.settingsProviderId) editProvider(state.settingsProviderId);
    else clearProviderForm();
    testAllProviders();
    await refreshCliStatus();
  });
}
