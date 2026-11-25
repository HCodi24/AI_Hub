import { useMemo } from "react";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { parseChatQuestions } from "@/lib/chat-questions";

/** MutiS: the app reads Codex's `mutis-delegate` blocks itself and shows the worker's result, so the raw JSON stays hidden. */
const hideDelegation = (text: string) => text.replace(/```mutis-delegate[\s\S]*?(?:```|$)/g, "").trim();

export function AiMessageText({ content, streaming }: { content: string; streaming: boolean }) {
  const text = useMemo(() => {
    const withoutDelegation = hideDelegation(content);
    if (streaming) return withoutDelegation.replace(/^```mutis-questions\s*$[\s\S]*$/m, "");
    return parseChatQuestions(withoutDelegation).text;
  }, [content, streaming]);
  return text ? <MarkdownView source={text} /> : null;
}
