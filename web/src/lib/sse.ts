import { http } from "@/lib/http";
import { chatText } from "@/text/chat";
export async function postSse(path: string, body: unknown, signal: AbortSignal,
  onEvent: (event: string, data: unknown) => void, terminal = "done"): Promise<void> {
  const response = await http.request(path, { method: "POST", body: JSON.stringify(body), signal });
  if (!response.body || !response.headers.get("content-type")?.includes("text/event-stream")) throw new Error(chatText.streamError);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let done = false;
  function dispatch(frame: string) {
    let event = "message";
    const data: string[] = [];
    for (const line of frame.split(/\r?\n/)) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
    }
    if (!data.length) return;
    onEvent(event, JSON.parse(data.join("\n")) as unknown);
    if (event === terminal) done = true;
  }
  try {
    while (!done) {
      const chunk = await reader.read();
      buffer += decoder.decode(chunk.value, { stream: !chunk.done });
      let separator: RegExpExecArray | null;
      while ((separator = /\r?\n\r?\n/.exec(buffer))) {
        dispatch(buffer.slice(0, separator.index));
        buffer = buffer.slice(separator.index + separator[0].length);
      }
      if (chunk.done) { if (buffer.trim()) dispatch(buffer); break; }
    }
    if (!done) throw new Error(chatText.streamError);
  } finally { await reader.cancel().catch(() => undefined); reader.releaseLock(); }
}
/** Lenient stream for command-style endpoints (dev test/build, extension actions): no terminal event is required and
 * `data` may be plain text instead of JSON. Resolves when the server closes the stream. */
export async function streamEvents(path: string, body: unknown, signal: AbortSignal,
  onEvent: (event: string, data: unknown) => void): Promise<void> {
  const response = await http.request(path, { method: "POST", signal, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  if (!response.body) throw new Error(chatText.streamError);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  function dispatch(frame: string) {
    let event = "message";
    const data: string[] = [];
    for (const line of frame.split(/\r?\n/)) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
    }
    if (!data.length) return;
    const text = data.join("\n");
    let value: unknown = text;
    try { value = JSON.parse(text) as unknown; } catch { /* plain text line */ }
    onEvent(event, value);
  }
  try {
    for (;;) {
      const chunk = await reader.read();
      buffer += decoder.decode(chunk.value, { stream: !chunk.done });
      let separator: RegExpExecArray | null;
      while ((separator = /\r?\n\r?\n/.exec(buffer))) {
        dispatch(buffer.slice(0, separator.index));
        buffer = buffer.slice(separator.index + separator[0].length);
      }
      if (chunk.done) { if (buffer.trim()) dispatch(buffer); break; }
    }
  } finally { await reader.cancel().catch(() => undefined); reader.releaseLock(); }
}
