import type { ChatMessage } from "@/schema/chat.schema";
import { chatChoices } from "./chat-choices";
import { parseChatQuestions } from "./chat-questions";
import { chatText } from "@/text/chat";

export type ChatPetState = "idle" | "working" | "replying" | "waiting" | "failed";
export function chatPetState(message: ChatMessage, streaming: boolean, latest: boolean): ChatPetState {
  if (message.role !== "assistant" || !latest) return "idle";
  if (message.error) return message.error === chatText.stopped ? "idle" : "failed";
  if (streaming) return message.content.trim() ? "replying" : "working";
  if (!message.plan_id && !message.debate_id && (chatChoices(message.content).length || parseChatQuestions(message.content).questions.length)) return "waiting";
  return "idle";
}
