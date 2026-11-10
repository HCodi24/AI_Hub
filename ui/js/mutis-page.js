import { $, el, showToast } from "./util.js";
import { state } from "./state.js";
import { api } from "./api.js";

export function statsTable(groups) {
  const table = el("table", { className: "plan-table" });
  table.innerHTML = "<thead><tr><th>Nhóm</th><th>Task</th><th>Thành công</th><th>Thời lượng TB</th><th>Token</th><th>Retry</th><th>Watchdog</th><th>Resume</th><th>Verify lỗi</th><th>Review lỗi</th><th>Sửa</th><th>Lỗi phổ biến</th></tr></thead>";
  const body = el("tbody");
  for (const [name, row] of Object.entries(groups || {})) body.append(el("tr", {}, [name, row.tasks_run, `${(row.success_rate * 100).toFixed(0)}%`, `${row.avg_duration_ms} ms`, row.total_tokens, row.retries, row.watchdog_kills, row.resumes, `${(row.verify_fail_rate * 100).toFixed(0)}%`, `${(row.review_fail_rate * 100).toFixed(0)}%`, row.fix_rounds, (row.top_errors || []).map(x => `${x.message} (${x.count})`).join("; ")].map(value => el("td", { textContent: String(value) }))));
  table.append(body); return table;
}
export async function loadMutisPage() {
  const stats = await api("/api/mutis/stats?days=30");
  const summary = $("#mutis-summary"); summary.replaceChildren();
  for (const [label, value] of [["Sự kiện", stats.events.length], ["Nhà cung cấp", Object.keys(stats.providers).length], ["Loại công việc", Object.keys(stats.task_kinds).length], ["Khoảng thời gian", `${stats.days} ngày`]]) summary.append(el("article", { className: "mutis-stat-card" }, [el("strong", { textContent: String(value) }), el("span", { textContent: label })]));
  $("#mutis-providers").replaceChildren(statsTable(stats.providers)); $("#mutis-kinds").replaceChildren(statsTable(stats.task_kinds));
  const events = stats.events || [], kinds = [...new Set(events.map(x => x.kind))], providers = [...new Set(events.map(x => x.provider_id).filter(Boolean))];
  const fill = (selector, vals) => { const select = $(selector), selected = select.value; select.replaceChildren(el("option", { value: "", textContent: selector.endsWith("kind") ? "Mọi loại" : "Mọi nhà cung cấp" })); vals.forEach(v => select.append(el("option", { value: v, textContent: v }))); select.value = selected; };
  fill("#mutis-filter-kind", kinds); fill("#mutis-filter-provider", providers);
  const render = () => { const filtered = events.filter(x => (!$("#mutis-filter-kind").value || x.kind === $("#mutis-filter-kind").value) && (!$("#mutis-filter-provider").value || x.provider_id === $("#mutis-filter-provider").value) && (!$("#mutis-filter-status").value || ($("#mutis-filter-status").value === "ok") === x.ok));
    const table = el("table", { className: "plan-table" }); table.innerHTML = "<thead><tr><th>Thời điểm</th><th>Loại</th><th>Provider</th><th>Task kind</th><th>Trạng thái</th><th>Chi tiết</th></tr></thead>"; const body = el("tbody"); filtered.forEach(x => body.append(el("tr", {}, [new Date(x.ts * 1000).toLocaleString(), x.kind, x.provider_id, x.task_kind || "—", x.ok ? "OK" : "Lỗi", x.detail || ""].map(value => el("td", { textContent: String(value) }))))); table.append(body); $("#mutis-events").replaceChildren(table); };
  ["#mutis-filter-kind", "#mutis-filter-provider", "#mutis-filter-status"].forEach(id => $(id).onchange = render); render();
  const diagnoses = await api("/api/mutis/diagnoses");
  const history = $("#mutis-diagnoses"); history.replaceChildren();
  for (const d of diagnoses) history.append(el("button", { className: "plan-list-item", textContent: `${new Date(d.ts * 1000).toLocaleString()} · ${d.health}`, onclick: () => renderDiagnosis(d) }));
  if (diagnoses.length && !$("#mutis-diagnosis-current").childElementCount) renderDiagnosis(diagnoses[0]);
}
export function settingChangeAllowed(change) {
  if (!change || typeof change.path !== "string" || !Object.hasOwn(change, "value")) return false;
  const path = change.path, value = change.value;
  const bools = ["retry_api", "codex_watchdog", "codex_auto_resume", "deepseek_agent", "verify_gate", "auto_review", "auto_fix", "event_log"];
  const ranges = { retry_attempts: [1, 5], retry_base_ms: [0, 60000], watchdog_minutes: [1, 60], resume_max: [0, 3], deepseek_max_file_lines: [50, 5000], deepseek_max_steps: [5, 40], fix_rounds_max: [0, 5] };
  if (path.startsWith("mutis.")) {
    const field = path.slice(6);
    if (bools.includes(field)) return typeof value === "boolean";
    if (ranges[field]) return Number.isInteger(value) && value >= ranges[field][0] && value <= ranges[field][1];
  }
  const roleMatch = path.match(/^mutis\.roles\.(lead|big|small)\.(provider_id|model|effort)$/);
  if (roleMatch) {
    if (roleMatch[2] === "provider_id") return state.config.providers.some(p => p.id === value);
    if (roleMatch[2] === "model") return typeof value === "string" && value.trim().length > 0 && value.length <= 200;
    return ["default", "low", "medium", "high", "xhigh", "max"].includes(value);
  }
  const routeMatch = path.match(/^mutis\.routing\[(\d+)\]\.role$/);
  return !!routeMatch && Number(routeMatch[1]) < (state.orchestration?.routing?.length || 0) && ["lead", "big", "small"].includes(value);
}
export function renderDiagnosis(diagnosis) {
  const root = $("#mutis-diagnosis-current");
  const badge = el("span", { className: `mutis-health mutis-health-${diagnosis.health === "tốt" ? "good" : diagnosis.health === "kém" ? "bad" : "medium"}`, textContent: diagnosis.health || "" });
  const card = el("article", { className: "mutis-diagnosis-result" }, [el("header", {}, [el("strong", { textContent: "Chẩn đoán MutiS" }), badge]), el("p", { textContent: diagnosis.summary || "" })]);
  for (const finding of diagnosis.findings || []) {
    const severity = el("span", { className: `mutis-health mutis-health-${finding.severity === "high" ? "bad" : finding.severity === "medium" ? "medium" : "good"}`, textContent: finding.severity || "" });
    const findingCard = el("article", { className: `mutis-finding severity-${finding.severity || "low"}` }, [el("header", {}, [el("strong", { textContent: finding.problem || "Phát hiện" }), severity]), el("p", { textContent: `Bằng chứng: ${finding.evidence || "—"}` }), el("p", { textContent: `Nguyên nhân: ${finding.cause || "—"}` }), el("p", { textContent: `Đề xuất: ${finding.suggestion || "—"}` })]);
    if (settingChangeAllowed(finding.setting_change)) {
      const change = finding.setting_change;
      const apply = el("button", { className: "subtle-button", textContent: "Áp dụng", onclick: async () => {
        try {
          const current = await api("/api/mutis"), old = getSettingValue(current, change.path);
          if (!window.confirm(`Đổi ${change.path}: ${JSON.stringify(old)} → ${JSON.stringify(change.value)}?`)) return;
          const result = await api("/api/mutis/apply", { method: "POST", body: JSON.stringify(change) });
          state.orchestration = await api("/api/mutis");
          showToast(`Đã đổi ${change.path}: ${JSON.stringify(result.old)} → ${JSON.stringify(result.new)}`);
        } catch (error) { showToast(error.message); }
      }});
      findingCard.append(apply);
    } else if (finding.setting_change) findingCard.append(el("p", { className: "muted", textContent: "Không áp dụng tự động được" }));
    card.append(findingCard);
  }
  root.replaceChildren(card);
}
export function getSettingValue(config, path) {
  if (path.startsWith("mutis.routing[")) { const index = Number(path.match(/^mutis\.routing\[(\d+)\]/)?.[1]); return config.routing?.[index]?.role; }
  const parts = path.replace(/^mutis\./, "").split(".");
  return parts.reduce((value, key) => value?.[key], config);
}
