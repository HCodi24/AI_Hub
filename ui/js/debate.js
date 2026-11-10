// "⚖ Tranh luận": Codex and Claude debate a request, a fresh Claude session judges (see src/features/debate.rs).
import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { folderInput } from "./chat.js";
import { api, readSse } from "./api.js";
import { planFromRequest } from "./plans.js";

const SIDE_NAMES = { codex: "Codex", claude: "Claude" };

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

function stanceBadge(turn) {
  if (turn.agree === true) return el("span", { className: "debate-badge agree", textContent: "Đồng ý" });
  if (turn.agree === false) return el("span", { className: "debate-badge disagree", textContent: "Chưa đồng ý" });
  return null;
}

function turnBox(side) {
  const box = el("div", { className: "debate-turn" });
  const head = el("div", { className: "debate-turn-head" }, [el("strong", { textContent: SIDE_NAMES[side] })]);
  const body = el("div", { className: "debate-text" });
  const meta = el("div", { className: "debate-meta" });
  box.append(head, body, meta);
  return { box, head, body, meta };
}

function fillTurn(view, turn) {
  view.body.textContent = turn.text || "";
  const badge = stanceBadge(turn);
  if (badge) view.head.append(badge);
  const bits = [];
  if (turn.tokens) bits.push(`${turn.tokens} token`);
  if (turn.disagreements?.length) bits.push(`Còn khác: ${turn.disagreements.join("; ")}`);
  if (turn.error) bits.push(`Lỗi: ${turn.error}`);
  view.meta.textContent = bits.join(" · ");
}

function roundSection(index) {
  const views = { codex: turnBox("codex"), claude: turnBox("claude") };
  const section = el("section", { className: "debate-round" }, [
    el("h3", { textContent: index === 1 ? "Vòng 1 · Đề xuất" : `Vòng ${index} · Phản biện` }),
    el("div", { className: "debate-columns" }, [views.codex.box, views.claude.box]),
  ]);
  return { section, views };
}

function list(title, items) {
  if (!items?.length) return [];
  return [el("h4", { textContent: title }), el("ul", {}, items.map(item => el("li", { textContent: item })))];
}

function verdictCard(verdict, debateId) {
  const chosen = { codex: "Chọn hướng của Codex", claude: "Chọn hướng của Claude", merged: "Kết hợp hai hướng" };
  const use = el("button", { className: "send-button wide", textContent: "Dùng kết quả này để chia việc" });
  use.onclick = async () => {
    try {
      const debate = await api(`/api/debates/${debateId}`);
      planFromRequest(debate.plan_request || "");
    } catch (error) { showToast(error.message); }
  };
  return el("section", { className: "debate-verdict" }, [
    el("h3", { textContent: `⚖ Phán quyết · ${chosen[verdict.chosen] || verdict.chosen || ""}` }),
    el("p", { className: "debate-text", textContent: verdict.decision || "" }),
    ...list("Hai bên đồng ý", verdict.agreed_points),
    ...list("Điểm còn tranh cãi", verdict.disagreements),
    ...list("Rủi ro", verdict.risks),
    ...list("Các bước", verdict.plan_steps),
    use,
  ]);
}

function header(debate) {
  return el("section", { className: "plan-progress" }, [
    el("h2", { textContent: `⚖ ${debate.title}` }),
    el("p", { className: "debate-meta", textContent:
      `Codex: ${debate.codex_model || debate.codex_provider} · Claude: ${debate.claude_model || debate.claude_provider}`
      + ` · Trọng tài (phiên Claude mới): ${debate.judge_model} · tối đa ${debate.max_rounds} vòng` }),
  ]);
}

/** Render a finished/stored debate. */
export function renderDebate(debate) {
  const root = showView();
  root.append(header(debate));
  for (const round of debate.rounds || []) {
    const { section, views } = roundSection(round.index);
    fillTurn(views.codex, round.codex);
    fillTurn(views.claude, round.claude);
    root.append(section);
  }
  if (debate.verdict) root.append(verdictCard(debate.verdict, debate.id));
  if (debate.error) root.append(el("p", { className: "debate-error", textContent: debate.error }));
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
  const status = el("p", { className: "debate-meta", textContent: "Đang bắt đầu…" });
  const controller = new AbortController();
  const stop = el("button", { className: "subtle-button", textContent: "Dừng", onclick: () => controller.abort() });
  root.append(el("section", { className: "plan-progress" }, [status, stop]));
  const rounds = new Map();
  let judgeBox = null;
  let debateId = null;
  try {
    const response = await fetch("/api/debates", { method: "POST", signal: controller.signal,
      headers: { "Content-Type": "application/json" }, body: JSON.stringify({ request, folder }) });
    if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
    await readSse(response.body, (event, data) => {
      if (event === "debate") { debateId = data.id; root.prepend(header(data)); }
      if (event === "round") {
        const view = roundSection(data.round);
        rounds.set(data.round, view);
        root.append(view.section);
        status.textContent = `Vòng ${data.round}: Codex và Claude đang suy nghĩ…`;
      }
      if (event === "delta") {
        if (data.side === "judge") { if (judgeBox) judgeBox.textContent += data.text; }
        else rounds.get(data.round)?.views[data.side]?.body.append(data.text || "");
      }
      if (event === "turn") {
        const view = rounds.get(data.round)?.views[data.side];
        if (view) fillTurn(view, data.turn);
      }
      if (event === "log" && data.side === "system") status.textContent = data.text;
      if (event === "judging") {
        status.textContent = `Trọng tài (phiên Claude thứ 2, ${data.model}) đang chấm…`;
        judgeBox = el("pre", { className: "task-log" });
        root.append(judgeBox);
      }
      if (event === "verdict") {
        judgeBox?.remove();
        root.append(verdictCard(data, debateId));
      }
      if (event === "error") root.append(el("p", { className: "debate-error", textContent: data.text || "Lỗi" }));
      if (event === "done") { status.textContent = data.status === "done" ? "Xong" : `Kết thúc: ${data.status}`; stop.remove(); }
    });
  } catch (error) {
    status.textContent = error.name === "AbortError" ? "Đã dừng" : error.message;
  } finally {
    if (button) button.disabled = false;
    renderDebateList();
  }
}
