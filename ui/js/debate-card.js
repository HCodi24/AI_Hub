// Shared renderer for live SSE and persisted debate records. Plain text avoids chat import cycles.
import { el, showToast } from "./util.js";
import { api } from "./api.js";

const names = { codex: "Codex", claude: "Claude" };
const clock = seconds => `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
export const debateCards = new Map();

// Markdown renderer injected by chat.js (avoids an import cycle); plain pre-wrap text until it is set.
let markdownRenderer = null;
export function setDebateMarkdown(render) { markdownRenderer = render; }
const formatTokens = (value) => Number(value || 0).toLocaleString("vi-VN");

export function createDebateCard({ live = false } = {}) {
  const root = el("section", { className: "debate-card" });
  let record = { status: "running", rounds: [], max_rounds: 2 };
  let running = live;
  let selected = 1;
  let phase = "round";
  let activeSide = "claude";
  let mobileSide = "codex";
  let started = Date.now();
  let timer = null;
  let planMade = false;
  const expanded = new Set();
  const activity = new Map();
  const turnStarts = new Map();
  const finished = new Set();
  const title = el("strong");
  const elapsed = el("span");
  const models = el("p", { className: "dc-meta" });
  const progress = el("div", { className: "dc-progress", ariaLabel: "Tiến trình tranh luận" });
  const verdict = el("section", { className: "dc-verdict" });
  const transcript = el("section");
  const tabs = el("div", { className: "dc-tabs", role: "tablist" });
  const toggle = el("div", { className: "dc-mobile-toggle" });
  const columns = el("div", { className: "dc-columns" });
  const status = el("p", { className: "dc-meta" });
  const stop = el("button", { textContent: "Dừng", onclick: () => root.dispatchEvent(new Event("debate-stop")) });
  root.append(el("header", {}, [title, elapsed]), models, progress, verdict, transcript, status, stop);
  transcript.append(tabs, toggle, columns);

  function tick() {
    elapsed.textContent = clock(running ? Math.floor((Date.now() - started) / 1000) : record.elapsed_s || 0);
  }
  function step(label, state) {
    const symbol = { done: "●", running: "◐", pending: "○", skipped: "○" }[state];
    return el("span", { className: `dc-step ${state}`, textContent: `${symbol} ${label}${state === "skipped" ? " · bỏ qua" : ""}` });
  }
  function renderProgress() {
    progress.replaceChildren();
    const last = record.rounds.at(-1);
    const converged = last?.codex?.agree === true && (last.index === 1 || last.claude?.agree === true);
    for (let i = 1; i <= (record.max_rounds || 2); i++) {
      const round = record.rounds.find(item => item.index === i);
      const done = round && (finished.has(`codex:${i}`) || (!running && round.codex?.text));
      const state = done ? "done" : running && phase === "round" && selectedRunning === i ? "running"
        : converged && i > last.index ? "skipped" : "pending";
      progress.append(step(`Vòng ${i} ${i === 1 ? "Đề xuất" : "Phản biện"}`, state));
    }
    progress.append(step("Trọng tài", record.verdict ? "done" : running && phase === "judge" ? "running" : "pending"));
    if (planMade) progress.append(step("Chia việc", "done"));
  }
  let selectedRunning = 1;
  function renderVerdict() {
    verdict.replaceChildren();
    verdict.hidden = !record.verdict;
    if (!record.verdict) return;
    const v = record.verdict;
    const chosen = { codex: "Chọn hướng Codex", claude: "Chọn hướng Claude", merged: "Kết hợp hai hướng" };
    verdict.append(el("h3", { textContent: `⚖ Phán quyết · ${chosen[v.chosen] || "Kết hợp hai hướng"}` }),
      el("p", { className: "dc-text", textContent: v.decision || "" }));
    const chips = el("div", { className: "dc-chips" });
    for (const [key, label] of [["agreed_points", "✔ Đồng ý"], ["disagreements", "⚠ Còn tranh cãi"], ["risks", "⚡ Rủi ro"], ["plan_steps", "▸"]]) {
      const items = v[key] || [];
      const details = el("details");
      details.append(el("summary", { textContent: key === "plan_steps" ? `▸ ${items.length} bước` : `${label} (${items.length})` }),
        el("ul", {}, items.map(textContent => el("li", { textContent }))));
      chips.append(details);
    }
    const use = el("button", { textContent: "✦ Chia việc từ phán quyết" });
    use.onclick = async () => {
      use.disabled = true;
      try {
        const debate = await api(`/api/debates/${encodeURIComponent(record.id)}`);
        const { folderInput } = await import("./chat.js");
        const { planFromRequest } = await import("./plans.js");
        folderInput.value = debate.folder;
        const plan = await planFromRequest(debate.plan_request || "");
        if (plan) {
          planMade = true;
          try { localStorage.setItem(`debate-plan-${record.id}`, plan.id); } catch (_) { /* Storage may be unavailable. */ }
          renderProgress();
        }
      } catch (error) { showToast(error.message); }
      finally { use.disabled = false; }
    };
    const minutes = el("button", { textContent: "Xem biên bản", onclick: () => {
      for (const round of record.rounds) for (const side of Object.keys(names)) expanded.add(`${side}:${round.index}`);
      renderRounds(); transcript.scrollIntoView({ behavior: "smooth", block: "nearest" });
    } });
    verdict.append(chips, el("div", { className: "dc-actions" }, [use, minutes]));
  }
  function sideBox(side, round) {
    const turn = round[side] || {};
    const key = `${side}:${round.index}`;
    const active = running && phase === "round" && selectedRunning === round.index && activeSide === side && !finished.has(key);
    const box = el("section", { className: `dc-side ${mobileSide === side ? "dc-selected-side" : ""}` });
    const head = el("div", { className: "dc-side-head" }, [el("strong", { textContent: names[side] })]);
    if (round.index > 1 && typeof turn.agree === "boolean") {
      head.append(el("span", { textContent: turn.agree ? "✅ Đồng ý" : "⚠ Chưa đồng ý" }));
    }
    const body = el("div", { className: `dc-text dc-body ${expanded.has(key) ? "expanded" : ""} ${active ? "streaming" : ""}`,
      textContent: turn.text || (active ? "Đang suy nghĩ…" : "Chưa có câu trả lời") });
    // Finished turns are rendered as markdown; the streaming one stays plain text (cheap, re-rendered per delta).
    if (markdownRenderer && turn.text && !active) {
      body.textContent = "";
      body.classList.add("dc-md");
      markdownRenderer(body, turn.text);
    }
    const words = (turn.text || "").trim().split(/\s+/u).filter(Boolean).length;
    const fold = el("button", { className: "dc-fold", textContent: expanded.has(key) ? "▾ Thu gọn" : `▸ Xem đầy đủ (${words} chữ)`,
      onclick: () => { if (expanded.has(key)) expanded.delete(key); else expanded.add(key); renderRounds(); } });
    box.append(head, body, fold);
    if (turn.disagreements?.length) box.append(el("p", { textContent: `Còn khác: ${turn.disagreements.join("; ")}` }));
    const seconds = active ? Math.floor((Date.now() - (turnStarts.get(key) || Date.now())) / 1000) : turn.elapsed_s || 0;
    box.append(el("p", { className: "dc-meta", textContent: `${formatTokens(turn.tokens)} token · ${clock(seconds)}` }));
    if (active) box.append(el("p", { className: "dc-activity", textContent: `● ${activity.get(key) || "Đang suy nghĩ…"}` }));
    if (turn.tokens > 500000) box.append(el("p", { className: "dc-warning", textContent: `⚠ ${names[side]} tốn nhiều token` }));
    if (turn.error) box.append(el("p", { className: "dc-error", textContent: turn.error }));
    if (active) requestAnimationFrame(() => { body.scrollTop = body.scrollHeight; });
    return box;
  }
  function renderRounds() {
    tabs.replaceChildren(); toggle.replaceChildren(); columns.replaceChildren();
    for (const round of record.rounds) tabs.append(el("button", { role: "tab", ariaSelected: String(selected === round.index),
      textContent: `Vòng ${round.index}`, onclick: () => { selected = round.index; renderRounds(); } }));
    for (const side of Object.keys(names)) toggle.append(el("button", { textContent: names[side], ariaPressed: String(mobileSide === side),
      onclick: () => { mobileSide = side; renderRounds(); } }));
    const round = record.rounds.find(item => item.index === selected);
    if (round) for (const side of Object.keys(names)) columns.append(sideBox(side, round));
  }
  function render() {
    title.textContent = `⚖ Tranh luận · ${record.title || "Đang bắt đầu…"}`;
    models.textContent = `Codex ${record.codex_model || "…"} · Claude ${record.claude_model || "…"} · Trọng tài: Claude mới (${record.judge_model || "…"})`;
    stop.hidden = !running;
    status.className = record.error ? "dc-error" : "dc-meta";
    status.textContent = record.error || (record.status === "running" && !live ? "đang chạy ở nơi khác / đã bị ngắt"
      : running && phase === "judge" ? `● Trọng tài đang chấm… ${activity.get("judge:0") || ""}` : record.status === "stopped" ? "Đã dừng" : "");
    tick(); renderProgress(); renderVerdict(); renderRounds();
    if (!running) { clearInterval(timer); timer = null; }
  }
  function load(data) {
    record = structuredClone(data);
    running = live && record.status === "running";
    selected = record.rounds?.at(-1)?.index || 1;
    record.rounds ||= [];
    try { planMade = Boolean(localStorage.getItem(`debate-plan-${record.id}`)); } catch (_) { /* Storage may be unavailable. */ }
    started = (record.created_at || Date.now() / 1000) * 1000;
    render();
  }
  function handle(name, data) {
    if (name === "debate") { load(data); debateCards.set(data.id, card); }
    else if (name === "round") {
      selected = selectedRunning = data.round; phase = "round"; activeSide = "claude"; mobileSide = "claude";
      record.rounds.push({ index: data.round, codex: {}, claude: {} });
      turnStarts.set(`claude:${data.round}`, Date.now());
    } else if (["delta", "log", "turn"].includes(name)) {
      const key = `${data.side}:${data.round || 0}`;
      const round = record.rounds.find(item => item.index === data.round);
      if (name === "log") {
        activity.set(key, data.text || "");
        const tokens = /^Token: vào (\d+).*?ra (\d+)/u.exec(data.text || "");
        if (tokens && round && names[data.side]) {
          round[data.side].tokens = (round[data.side].tokens || 0) + Number(tokens[1]) + Number(tokens[2]);
        }
      }
      if (round && names[data.side]) {
        if (name === "delta") round[data.side].text = (round[data.side].text || "") + (data.text || "");
        if (name === "turn") {
          round[data.side] = data.turn; finished.add(key);
          if (data.side === "claude") { activeSide = "codex"; mobileSide = "codex"; turnStarts.set(`codex:${data.round}`, Date.now()); }
        }
      }
    } else if (name === "judging") { phase = "judge"; record.judge_model = data.model; }
    else if (name === "verdict") record.verdict = data;
    else if (name === "error") record.error = data.text || "Lỗi tranh luận";
    else if (name === "done") {
      running = false;
      record = { ...record, ...data, elapsed_s: data.elapsed_s ?? Math.floor((Date.now() - started) / 1000) };
    }
    render();
  }
  const card = { el: root, handle, load };
  if (live) timer = setInterval(() => { tick(); if (root.isConnected) renderRounds(); }, 1000);
  render();
  return card;
}

export function storedDebateCard(id) {
  if (debateCards.has(id)) return debateCards.get(id);
  const card = createDebateCard({ live: false });
  debateCards.set(id, card);
  api(`/api/debates/${encodeURIComponent(id)}`).then(data => card.load(data)).catch(error => {
    debateCards.delete(id); card.handle("error", { text: error.message });
  });
  return card;
}
