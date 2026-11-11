import { memo } from "react";
import { Virtuoso } from "react-virtuoso";
import { EmptyState } from "@/components/empty-state/empty-state";
import { ChatMessage } from "./chat-message";
import { useChatStore } from "./chat-store";
const MessageRow = memo(function MessageRow({ index }: { index: number }) {
  const message = useChatStore(s => s.messages[index]);
  const id = useChatStore(s => s.id);
  const streaming = useChatStore(s => s.streaming && index === s.messages.length - 1);
  return message ? <ChatMessage message={message} chatId={id} streaming={streaming} /> : null;
});
export const MessageList = memo(function MessageList() {
  const count = useChatStore(s => s.messages.length);
  const id = useChatStore(s => s.id);
  return count ? <Virtuoso key={id} totalCount={count} followOutput="auto" initialTopMostItemIndex={{ index: count - 1, align: "end" }}
    itemContent={index => <MessageRow index={index} />} /> : <EmptyState />;
});
