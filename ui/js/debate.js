import { $, el, showToast } from "./util.js";
import { api } from "./api.js";
import { state } from "./state.js";
import { debateInChat, loadChat, newChat, folderInput, showChatPanel, refreshChats } from "./chat.js";

export async function openDebate(id) {
  if (state.streaming) return;
  try {
    const record = await api(`/api/debates/${encodeURIComponent(id)}`);
    if (!record.chat_id) {
      newChat(); folderInput.value = record.folder;
      await api(`/api/debates/${encodeURIComponent(id)}/chat`, {
        method: "POST", body: JSON.stringify({ chat_id: state.chatId }),
      });
      record.chat_id = state.chatId;
      await refreshChats();
    }
    await loadChat(record.chat_id);
    showChatPanel();
    await renderDebateList();
  } catch (error) { showToast(error.message); }
}
export async function renderDebateList() {
  const root = $("#legacy-debates");
  if (!root) return;
  const records = await api("/api/debates");
  root.replaceChildren();
  for (const record of records.filter(item => !item.chat_id)) {
    root.append(el("button", { className: "plan-list-item", textContent: `⚖ ${record.title}`, onclick: () => openDebate(record.id) }));
  }
  $("#legacy-records").hidden = !root.childElementCount && !$("#legacy-plans").childElementCount;
}
export async function debateFromRequest(text) {
  const request = text.trim() || [...state.messages].reverse().find(message => message.role === "user")?.content || "";
  if (!request || state.streaming) return;
  await debateInChat(request);
}
