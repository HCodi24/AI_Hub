import { forwardRef, memo, useMemo, type HTMLAttributes, type Ref } from "react";
import { Virtuoso } from "react-virtuoso";
import { EmptyState } from "@/components/empty-state/empty-state";
import { ScrollArea } from "@/components/scroll-area";
import { ChatMessage } from "./chat-message";
import { useChatStore } from "./chat-store";
import { QuickReplies, type SendReply } from "./quick-replies";
import { parseChatQuestions } from "@/lib/chat-questions";
import { ChatQuestions } from "./chat-questions";

/** Virtuoso scroller adapter: renders the shared themed ScrollArea and drops Virtuoso's context prop. */
const Scroller = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement> & { context?: unknown }>(function Scroller(
  { context: _context, ...props },
  ref,
) {
  return <ScrollArea ref={ref as Ref<HTMLElement>} {...props} />;
});

/** Stable module-level components map so Virtuoso does not remount its scroller on every render. */
const SCROLL_COMPONENTS = { Scroller };

const MessageRow = memo(function MessageRow({ index, send }: { index: number; send: SendReply }) {
  const message = useChatStore(s => s.messages[index]);
  const id = useChatStore(s => s.id);
  const streaming = useChatStore(s => s.streaming && index === s.messages.length - 1);
  const latest = useChatStore(s => index === s.messages.length - 1);
  const status = useChatStore(s => s.streaming && index === s.messages.length - 1 ? s.status : undefined);
  const questions = useMemo(() => message?.role === "assistant" ? parseChatQuestions(message.content).questions : [], [message?.role, message?.content]);
  const canReply = message?.role === "assistant" && !streaming && !message.error && !message.plan_id && !message.debate_id;
  return message ? <ChatMessage message={message} chatId={id} streaming={streaming} latest={latest} status={status}
    replies={canReply && questions.length > 0
      ? <ChatQuestions key={`${id}:${index}`} questions={questions} send={send} active={latest && !streaming} chatId={id} />
      : latest && canReply ? <QuickReplies key={`${id}:${index}`} content={message.content} send={send} /> : undefined} /> : null;
});
export const MessageList = memo(function MessageList({ send }: { send: SendReply }) {
  const count = useChatStore(s => s.messages.length);
  const id = useChatStore(s => s.id);
  return count ? <Virtuoso key={id} totalCount={count} followOutput="auto" initialTopMostItemIndex={{ index: count - 1, align: "end" }}
    components={SCROLL_COMPONENTS}
    itemContent={index => <MessageRow index={index} send={send} />} /> : <EmptyState />;
});
