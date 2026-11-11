import { DebateSchema, DebateEventSchema, TurnSchema, RoundSchema, VerdictSchema, type Debate } from "@/schema/debates.schema";
export type DebateLive = { record: Debate; activity: string; phase: string; round: number; live: boolean; finished?: string[] };
export function debateReducer(state: DebateLive, event: string, raw: unknown): DebateLive {
  if (event === "debate") return { ...state, record: DebateSchema.parse(raw) };
  if (event === "done") return { ...state, record: DebateSchema.parse(raw), live: false };
  if (event === "verdict") return { ...state, record: { ...state.record, verdict: VerdictSchema.parse(raw) } };
  const data = DebateEventSchema.parse(raw);
  const next = { ...state, record: structuredClone(state.record) };
  if (event === "round" && data.round) {
    next.round = data.round; next.phase = "round";
    if (!next.record.rounds.some(r => r.index === data.round)) next.record.rounds.push(RoundSchema.parse({ index: data.round }));
  }
  if (event === "judging") { next.phase = "judge"; next.record.judge_model = data.model || ""; }
  if (event === "log") next.activity = data.text || "";
  if (event === "error") next.record.error = data.text;
  const round = next.record.rounds.find(r => r.index === data.round);
  if (round && (data.side === "codex" || data.side === "claude")) {
    if (event === "delta") round[data.side].text += data.text || "";
    if (event === "turn") { round[data.side] = TurnSchema.parse(data.turn); next.finished = [...(state.finished || []), `${data.side}:${data.round}`]; }
    const usage = event === "log" && /^Token: vào (\d+).*?ra (\d+)/u.exec(data.text || "");
    if (usage) round[data.side].tokens += Number(usage[1]) + Number(usage[2]);
  }
  return next;
}
