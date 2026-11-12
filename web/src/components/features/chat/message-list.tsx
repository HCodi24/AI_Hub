import { memo } from "react";
import { Virtuoso } from "react-virtuoso";
import { EmptyState } from "@/components/empty-state/empty-state";
import { ChatMessage } from "./chat-message";
import { useChatStore } from "./chat-store";
import { QuickReplies, type SendReply } from "./quick-replies";
const MessageRow = memo(function MessageRow({ index, send }: { index: number; send: SendReply }) {
  const message = useChatStore(s => s.messages[index]);
  const id = useChatStore(s => s.id);
  const streaming = useChatStore(s => s.streaming && index === s.messages.length - 1);
  const latest = useChatStore(s => index === s.messages.length - 1);
  return message ? <ChatMessage message={message} chatId={id} streaming={streaming}
    replies={latest && message.role === "assistant" && !message.error && !message.plan_id && !message.debate_id
      ? <QuickReplies key={`${id}:${index}`} content={message.content} send={send} /> : undefined} /> : null;
});
export const MessageList = memo(function MessageList({ send }: { send: SendReply }) {
  const count = useChatStore(s => s.messages.length);
  const id = useChatStore(s => s.id);
  return count ? <Virtuoso key={id} totalCount={count} followOutput="auto" initialTopMostItemIndex={{ index: count - 1, align: "end" }}
    itemContent={index => <MessageRow index={index} send={send} />} /> : <EmptyState />;
});
