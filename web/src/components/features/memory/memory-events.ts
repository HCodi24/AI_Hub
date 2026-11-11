import { MemoryEventSchema } from "@/schema/memory.schema";
import { memoryApi } from "@/apis/memory.api";
import { cardText as t } from "@/text/cards";
export function memoryNotice(raw: unknown, folder: string) {
  const event = MemoryEventSchema.parse(raw);
  return { text: event.action === "archived" ? t.archiveToast(event.scope) : t.memoryToast(event.scope),
    action: event.action === "archived" ? () => memoryApi.restore(folder, event.scope) : undefined };
}
