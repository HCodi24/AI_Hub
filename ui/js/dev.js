import { $, el } from "./util.js";
import { state } from "./state.js";
import { api, readSse } from "./api.js";

export function showDevBanner(message, action) {
  const root = $("#dev-banner"); root.replaceChildren(); root.hidden = false;
  root.classList.toggle("dev-banner-restarting", message.includes("khởi động lại"));
  root.append(el("span", { textContent: message }));
  if (action) { const button = el("button", { className: "subtle-button", textContent: "Build & khởi động lại" }); button.onclick = startDevBuild; root.append(button); }
}
let reloadTimer = null;
// Remember the open chat so the page comes back to it after a dev reload instead of starting a new chat.
function reloadPage() {
  try { if (state.chatId) sessionStorage.setItem("ai-hub-resume-chat", state.chatId); } catch (_) { /* storage unavailable */ }
  location.reload();
}
// Reloading the page drops the running chat request, and the server then cancels the AI run. When the AI edits
// ui/ files itself that would kill its own session, so wait until no chat is streaming before reloading.
function reloadWhenIdle() {
  if (!state.streaming) { reloadPage(); return; }
  showDevBanner("Giao diện đã đổi · sẽ tự tải lại khi AI chạy xong", false);
  if (reloadTimer) return;
  reloadTimer = setInterval(() => {
    if (state.streaming) return;
    clearInterval(reloadTimer);
    reloadTimer = null;
    setTimeout(reloadPage, 800);
  }, 1000);
}
export function connectDevEvents() {
  const source = new EventSource("/api/dev/events");
  source.addEventListener("ui_changed", reloadWhenIdle);
  source.addEventListener("rust_changed", event => { const files=JSON.parse(event.data||"{}").files||[]; showDevBanner("Code Rust đã thay đổi ("+files.length+" tệp) · ", true); });
  source.addEventListener("rebuild", event => {
    const result = JSON.parse(event.data || "{}");
    showDevBanner(
      result.ok ? "Đang khởi động lại…" : "Build lỗi:\n" + (result.errors_tail || "Không có chi tiết lỗi"),
      false,
    );
  });
  source.addEventListener("build_log", event => {
    const data = JSON.parse(event.data || "{}");
    const root = $("#dev-banner");
    root.hidden = false;
    let log = root.querySelector("pre");
    if (!log) {
      log = el("pre", { className: "dev-build-log" });
      root.append(log);
    }
    log.textContent = (log.textContent + data.text).slice(-12000);
  });
  source.onerror = async () => {
    source.close();
    showDevBanner("Đang khởi động lại…", false);
    while (true) {
      await new Promise(resolve => setTimeout(resolve, 1000));
      try {
        const status = await api("/api/dev/status");
        if (status.dev) {
          reloadWhenIdle();
          return;
        }
      } catch (_) {}
    }
  };
}
export async function refreshDevRepo() {
  try { state.devRepo=await api("/api/dev/repo"); const r=state.devRepo, root=$("#dev-repo-info"); root.replaceChildren();
    for (const [label, value] of [
      ["Đường dẫn", r.path],
      ["Nhánh", r.branch || "(detached)"],
      ["File thay đổi", r.changed_files],
      ["Commit cuối", `${r.last_commit.hash} · ${r.last_commit.subject} · ${r.last_commit.author}`],
    ]) {
      root.append(el("p", {}, [
        el("strong", { textContent: `${label}: ` }),
        document.createTextNode(String(value)),
      ]));
    }
  } catch(error) { $("#dev-repo-info").textContent=error.message; }
}
export function showDevPage() {
  $(".main-panel").hidden = true;
  $("#mutis-view").hidden = true;
  $("#plans-view").hidden = true;
  $("#self-dev-view").hidden = false;
  refreshDevRepo();
  clearInterval(state.devRepoTimer);
  state.devRepoTimer = setInterval(refreshDevRepo, 10000);
}
export function hideDevPage() { $("#self-dev-view").hidden=true; $(".main-panel").hidden=false; clearInterval(state.devRepoTimer); }
export async function devAction(path, body) { return api(path,{method:body?"POST":"GET",...(body?{body:JSON.stringify(body)}:{})}); }
export function renderDevDiff(data) {
  const root = $("#dev-git-result");
  root.replaceChildren(el("p", { textContent: data.stat || "Không có diff được theo dõi." }));
  if (data.untracked?.length) {
    root.append(el("p", { textContent: `File chưa theo dõi:\n${data.untracked.join("\n")}` }));
  }
  const pre = el("pre", { className: "dev-diff" });
  for (const line of data.diff.split("\n")) {
    const className = line.startsWith("+") && !line.startsWith("+++")
      ? "diff-add"
      : line.startsWith("-") && !line.startsWith("---")
        ? "diff-remove"
        : "";
    const span = el("span", { className, textContent: line + "\n" });
    pre.append(span);
  }
  root.append(pre);
  if (data.truncated) {
    root.append(el("p", { textContent: "Diff đã giới hạn ở 200 KB." }));
  }
}
export async function streamDevRun(url, target) {
  target.hidden = false;
  target.textContent = "";
  try {
    const response = await fetch(url, { method: "POST" });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    await readSse(response.body, (event, data) => {
      if (event === "log") target.textContent += `${data.text || ""}\n`;
      if (event === "summary" && Object.hasOwn(data, "cargo_ok")) target.textContent += `${data.cargo_ok ? "✓" : "✗"} cargo test ${data.cargo_ok ? "đạt" : "thất bại"}\n`;
      if (event === "verify") target.textContent += `${data.ok ? "✓" : "✗"} MutiS verify: ${(data.summary_lines || []).join(" · ")}\n`;
      if (event === "summary" && Object.hasOwn(data, "exe")) target.textContent += `${data.ok ? "✓ Build thành công" : "✗ Build thất bại"}${data.ok ? `\nExe: ${data.exe}\n${data.note}` : ""}\n`;
      target.scrollTop = target.scrollHeight;
    });
  } catch (error) { target.textContent += `${error.message}\n`; }
}
export async function startDevBuild() {
  showDevBanner("Đang build…",false);
  try { const response=await fetch("/api/dev/rebuild",{method:"POST"}); if(!response.ok){const error=await response.json();showDevBanner(error.error||"Không thể build",false);return;}
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    while (true) {
      const item = await reader.read();
      if (item.done) break;
      buffer += decoder.decode(item.value, { stream: true });
      const chunks = buffer.split("\n\n");
      buffer = chunks.pop();
      for (const chunk of chunks) {
        const event = chunk.match(/^event: (.+)$/m)?.[1];
        const data = chunk.match(/^data: (.+)$/m)?.[1];
        if (event === "rebuild" && data) {
          const result = JSON.parse(data);
          showDevBanner(
            result.ok ? "Đang khởi động lại…" : "Build lỗi:\n" + result.errors_tail,
            false,
          );
        }
      }
    }
  } catch(error) { showDevBanner(error.message,false); }
}
