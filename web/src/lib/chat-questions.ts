import { z } from "zod";

const optionSchema = z.object({
  label: z.string().trim().min(1).max(200),
  description: z.string().max(1000).optional(),
  recommended: z.boolean().optional(),
}).strict();
const questionSchema = z.object({
  id: z.string().trim().min(1).max(80),
  question: z.string().trim().min(1).max(1000),
  options: z.array(optionSchema).max(5),
}).strict().refine(q => new Set(q.options.map(option => option.label)).size === q.options.length);
const payloadSchema = z.object({ questions: z.array(questionSchema).min(1).max(3) }).strict()
  .refine(value => new Set(value.questions.map(q => q.id)).size === value.questions.length);

export type ChatQuestion = z.infer<typeof questionSchema>;

/** Recognize only a top-level fenced card; preserve invalid payloads as ordinary text. */
export function parseChatQuestions(content: string): { text: string; questions: ChatQuestion[] } {
  const fallback = { text: content, questions: [] };
  const masked = content.replace(/<memory\b[^>]*>[\s\S]*?(?:<\/memory\s*>|$)/gi,
    block => block.replace(/[^\r\n]/g, " "));
  const cards: { start: number; end: number; json: string }[] = [];
  let fence: { char: string; length: number; card: boolean; start: number; body: number } | null = null;
  const lines = masked.matchAll(/[^\n]*(?:\n|$)/g);
  for (const match of lines) {
    const line = match[0].replace(/\r?\n$/, "");
    const mark = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (!mark) continue;
    if (!fence) {
      fence = { char: mark[1][0], length: mark[1].length, card: mark[1][0] === "`" && mark[2].trim() === "mutis-questions",
        start: match.index!, body: match.index! + match[0].length };
    } else if (mark[1][0] === fence.char && mark[1].length >= fence.length && !mark[2].trim()) {
      if (fence.card) cards.push({ start: fence.start, end: match.index! + line.length, json: content.slice(fence.body, match.index!) });
      fence = null;
    }
  }
  if (cards.length !== 1 || fence?.card || cards[0].json.length > 25000) return fallback;
  try {
    const result = payloadSchema.safeParse(JSON.parse(cards[0].json));
    if (!result.success) return fallback;
    return { text: content.slice(0, cards[0].start) + content.slice(cards[0].end), questions: result.data.questions };
  } catch { return fallback; }
}

export function formatQuestionAnswers(questions: ChatQuestion[], answers: Record<string, string>): string {
  return questions.map((q, index) => `${index + 1}. ${q.question}\n${answers[q.id]?.trim() || ""}`).join("\n\n");
}
