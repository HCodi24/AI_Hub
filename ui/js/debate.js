// "⚖ Tranh luận": Codex and Claude debate a request, a fresh Claude session judges (see src/features/debate.rs).
import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { folderInput } from "./chat.js";
import { api, readSse } from "./api.js";
import { createDebateCard, debateCards } from "./debate-card.js";


function showView() {
  $(".main-panel").hidden = true;
  $("#self-dev-view").hidden = true;
  $("#plans-view").hidden = false;
  $("#plan-editor").hidden = true;
  const root = $("#plans-list");
  root.hidden = false;
  root.replaceChildren();
  return root;
}

/** Use the same cached card as chat, including a live card if one exists. */
export function renderDebate(debate) {
  let card = debateCards.get(debate.id);
  if (!card) {
    card = createDebateCard({ live: false });
    card.load(debate);
    debateCards.set(debate.id, card);
  }
  showView().append(card.el);
}

export async function openDebate(id) {
  try { renderDebate(await api(`/api/debates/${id}`)); } catch (error) { showToast(error.message); }
}

export async function renderDebateList() {
  const root = $("#debates-list");
  if (!root) return;
  let debates = [];
  try { debates = await api("/api/debates"); } catch (_) { /* keep empty */ }
  root.replaceChildren();
  if (!debates.length) { root.textContent = "Chưa có tranh luận."; return; }
  for (const debate of debates) {
    const mark = debate.status === "done" ? "✓" : debate.status === "running" ? "…" : "!";
    root.append(el("button", { className: "plan-list-item", textContent: `${mark} ${debate.title}`,
      onclick: () => openDebate(debate.id) }));
  }
}

/** Start a live debate on `text` (or the last user message) in the chat folder. */
export async function debateFromRequest(text) {
  const request = text.trim() || [...state.messages].reverse().find(message => message.role === "user")?.content || "";
  if (!request) { showToast("Nhập vấn đề hoặc mở chat có yêu cầu trước."); return; }
  const folder = folderInput.value.trim();
  if (!folder) { showToast("Chọn thư mục làm việc trước khi tranh luận."); return; }
  const button = $("#debate-button");
  if (button) button.disabled = true;
  const root = showView();
  const card = createDebateCard({ live: true });
  const controller = new AbortController();
  card.el.addEventListener("debate-stop", () => controller.abort());
  root.append(card.el);
  let receivedDone = false;
  try {
    const response = await fetch("/api/debates", { method: "POST", signal: controller.signal,
      headers: { "Content-Type": "application/json" }, body: JSON.stringify({ request, folder }) });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    await readSse(response.body, (event, data) => {
      card.handle(event, data);
      if (event === "done") receivedDone = true;
    });
    if (!receivedDone) throw new Error("Mất kết nối khi tranh luận");
  } catch (error) {
    card.handle("error", { text: error.name === "AbortError" ? "Đã dừng" : error.message });
    card.handle("done", { status: error.name === "AbortError" ? "stopped" : "error" });
  } finally {
    if (button) button.disabled = false;
    renderDebateList();
  }
}
