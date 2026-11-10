import { $, el, showToast } from "./util.js";
import { api } from "./api.js";
import { state } from "./state.js";

export function hideMemory(text) {
  return text.replace(/<memory>[\s\S]*?(?:<\/memory>|$)/g, "");
}
const query = (folder) => `?folder=${encodeURIComponent(folder)}`;
const scopeUrl = (folder, scope, action = "") => `/api/memory/${encodeURIComponent(scope)}${action}${query(folder)}`;
const folderNow = () => state.devChat && state.devRepo?.path ? state.devRepo.path : $("#folder-input").value;

export function memoryEvent(data, folder = folderNow()) {
  showToast(data.action === "archived" ? `Đã đóng scope ${data.scope}` : `📝 Đã cập nhật bộ nhớ: ${data.scope}`);
  if (data.action === "archived") {
    const undo = el("button", { type: "button", textContent: "Hoàn tác" });
    undo.addEventListener("click", async () => {
      try { await api(scopeUrl(folder, data.scope, "/restore"), { method: "POST" }); showToast("Đã khôi phục scope"); }
      catch (error) { showToast(error.message); }
    });
    $("#toast").append(" ", undo);
  }
}

async function openMemory() {
  const folder = folderNow();
  const dialog = el("dialog", { className: "memory-dialog" });
  const close = el("button", { type: "button", textContent: "Đóng" });
  close.onclick = () => dialog.close();
  dialog.addEventListener("close", () => dialog.remove());
  const list = el("div", { className: "memory-list" });
  const editor = el("textarea", { className: "memory-editor", ariaLabel: "Nội dung scope" });
  editor.hidden = true;
  const actions = el("div", { className: "memory-actions" });
  dialog.append(el("h2", { textContent: "📝 Bộ nhớ dự án" }), el("p", { textContent: folder }), close, list, editor, actions);
  document.body.append(dialog);
  dialog.showModal();
  const run = async (fn) => { try { await fn(); } catch (error) { showToast(error.message); } };
  const refresh = async () => {
    const view = await api(`/api/memory${query(folder)}`);
    list.replaceChildren(); actions.replaceChildren(); editor.hidden = true;
    for (const [heading, scopes, archived] of [["Đang làm", view.scopes, false], ["Đã lưu trữ", view.archived, true]]) {
      list.append(el("h3", { textContent: heading }));
      for (const scope of scopes) {
        const button = el("button", { type: "button", textContent: `[${scope.status}] ${scope.scope} — ${scope.title}` });
        button.onclick = () => run(async () => {
          const result = await api(scopeUrl(folder, scope.scope));
          editor.value = result.text; editor.hidden = false; actions.replaceChildren();
          const action = (label, fn) => {
            const b = el("button", { type: "button", textContent: label });
            b.onclick = () => run(fn); actions.append(b);
          };
          action("Lưu", async () => {
            const saved = await api(scopeUrl(folder, scope.scope), { method: "PUT", body: JSON.stringify({ text: editor.value }) });
            editor.value = saved.text; showToast("Đã lưu bộ nhớ"); await refresh();
          });
          action(archived ? "Khôi phục" : "Lưu trữ", async () => {
            await api(scopeUrl(folder, scope.scope, archived ? "/restore" : "/archive"), { method: "POST" }); await refresh();
          });
          if (archived) action("Xoá", async () => {
            if (!confirm(`Xoá vĩnh viễn scope ${scope.scope}?`)) return;
            await api(scopeUrl(folder, scope.scope), { method: "DELETE" }); await refresh();
          });
          editor.focus();
        });
        list.append(button);
      }
    }
  };
  await run(refresh);
}

export function renderMemorySettings() {
  const config = state.config.memory || { enabled: true, max_scopes_in_prompt: 3, max_chars_per_scope: 6000 };
  let section = $("#memory-settings");
  if (!section) {
    section = el("section", { id: "memory-settings", className: "planner-settings" });
    section.append(el("h3", { textContent: "Bộ nhớ dự án" }));
    for (const [key, title, type, min, max] of [
      ["enabled", "Bật bộ nhớ dự án", "checkbox"],
      ["max_scopes_in_prompt", "Số scope trong ngữ cảnh", "number", 1, 10],
      ["max_chars_per_scope", "Số ký tự tối đa mỗi scope", "number", 1000, 20000],
    ]) {
      const input = el("input", { type, id: `memory-${key}` });
      if (min) { input.min = min; input.max = max; }
      input.addEventListener("change", async () => {
        const memory = {
          enabled: $("#memory-enabled").checked,
          max_scopes_in_prompt: Number($("#memory-max_scopes_in_prompt").value),
          max_chars_per_scope: Number($("#memory-max_chars_per_scope").value),
        };
        try {
          state.config = await api("/api/config", { method: "PUT", body: JSON.stringify({ default_folder: state.config.default_folder, memory }) });
          showToast("Đã lưu cấu hình bộ nhớ");
        } catch (error) { showToast(error.message); renderMemorySettings(); }
      });
      section.append(el("label", {}, [title, input]));
    }
    $("#settings-dialog .settings-content").prepend(section);
  }
  for (const [key, value] of Object.entries(config)) {
    const input = $(`#memory-${key}`);
    if (input) { if (input.type === "checkbox") input.checked = value; else input.value = value; }
  }
}

const memoryButton = el("button", { type: "button", className: "subtle-button", textContent: "📝 Bộ nhớ" });
memoryButton.onclick = openMemory;
$("#debate-button").after(memoryButton);
