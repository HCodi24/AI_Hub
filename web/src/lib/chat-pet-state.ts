import type { ChatMessage } from "@/schema/chat.schema";
import { chatChoices } from "./chat-choices";
import { parseChatQuestions } from "./chat-questions";
import { chatText } from "@/text/chat";

export type ChatPetState = "idle" | "thinking" | "working" | "replying" | "waiting" | "failed";
const COMMAND = /^(?:Command|Lệnh):/i;
/** shortcut: reasoning text exists and the newest log is not a command; a stream flag would be exact, add one if this misfires. */
const thinking = (message: ChatMessage) => Boolean(message.reasoning) && !COMMAND.test(message.logs.at(-1) ?? "");
export function chatPetState(message: ChatMessage, streaming: boolean, latest: boolean): ChatPetState {
  if (message.role !== "assistant" || !latest) return "idle";
  if (message.error) return message.error === chatText.stopped ? "idle" : "failed";
  if (streaming) return message.content.trim() ? "replying" : thinking(message) ? "thinking" : "working";
  if (!message.plan_id && !message.debate_id && (chatChoices(message.content).length || parseChatQuestions(message.content).questions.length)) return "waiting";
  return "idle";
}
