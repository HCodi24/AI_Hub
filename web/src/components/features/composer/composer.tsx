import { useEffect, useRef, useState } from "react";
import { z } from "zod";
import { ArrowUp, Square, Scale, Sparkles, ImagePlus, Plus, X, FileEdit } from "lucide-react";
import { Button } from "@/components/button/button";
import { http } from "@/lib/http";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
import { startCard } from "@/components/features/chat/card-actions";
import { notify } from "@/components/toast/toast";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { ChatToolbar } from "./chat-toolbar";
import { MemoryPanel } from "@/components/features/memory/memory-panel";
import type { Config } from "@/schema/config.schema";
import styles from "./composer.module.css";
const MAX_IMAGES = 5;
const UploadSchema = z.object({ id: z.string(), mime: z.string(), url: z.string() });
type Image = z.infer<typeof UploadSchema>;
const readBase64 = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = () => reject(reader.error);
  reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
  reader.readAsDataURL(file);
});
export function Composer({ config, streaming, send, stop }: { config?: Config; streaming: boolean; send: (text: string, direct?: boolean, attachments?: { id: string; mime: string }[]) => Promise<boolean>; stop: () => Promise<void> }) {
  const [debate, setDebate] = useState(false);
  const running = useCardStore(s => s.running);
  const chatId = useChatStore(s => s.id);
  const folder = useChatStore(s => s.settings.folder);
  const [memory, setMemory] = useState(false);
  useEffect(() => { try { setDebate(["1", "true"].includes(localStorage.getItem("debateMode") || "")); } catch { /* Storage unavailable. */ } }, []);
  const [value, setValue] = useState("");
  const [images, setImages] = useState<Image[]>([]);
  const input = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const attachmentMenu = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      if (attachmentMenu.current?.open && !attachmentMenu.current.contains(event.target as Node)) attachmentMenu.current.open = false;
    }
    document.addEventListener("pointerdown", closeOutside, true);
    return () => document.removeEventListener("pointerdown", closeOutside, true);
  }, []);
  const pending = useRef(false);
  // Uploads are stored per chat id, so images picked for another chat no longer apply.
  const owner = useRef(chatId);
  useEffect(() => { if (owner.current !== chatId) { owner.current = chatId; setImages([]); } }, [chatId]);
  useEffect(() => {
    if (input.current) { input.current.style.height = "auto"; input.current.style.height = `${Math.min(input.current.scrollHeight, 180)}px`; }
  }, [value]);
  useEffect(() => {
    function suggest(event: Event) {
      if (!(event instanceof CustomEvent) || typeof event.detail !== "string") return;
      setValue(event.detail); input.current?.focus();
    }
    window.addEventListener("ai-hub:suggestion", suggest);
    return () => window.removeEventListener("ai-hub:suggestion", suggest);
  }, []);
  async function addImages(files: File[]) {
    let count = images.length;
    for (const file of files) {
      if (!file.type.startsWith("image/")) continue;
      if (count >= MAX_IMAGES) { notify(chatText.maxImages); break; }
      let id = useChatStore.getState().id;
      if (!id) { id = crypto.randomUUID(); useChatStore.setState({ id }); owner.current = id; }
      try {
        const data = await readBase64(file);
        const response = await http.request("/attachments", { method: "POST", body: JSON.stringify({ chat_id: id, data }) });
        // Preview from the local bytes: it shows even if the stored copy cannot be fetched back.
        const image = { ...UploadSchema.parse(await response.json()), url: `data:${file.type};base64,${data}` };
        if (useChatStore.getState().id === id) { count++; setImages(current => [...current, image]); }
      } catch (error) { notify(error instanceof Error ? error.message : t.error); }
    }
    if (picker.current) picker.current.value = "";
  }
  const canSend = Boolean(value.trim()) || images.length > 0;
  async function submit() {
    if (pending.current || streaming || !canSend) return;
    pending.current = true;
    const draft = value, sent = images; setValue(""); setImages([]);
    try { if (!(await send(draft.trim(), false, sent.map(({ id, mime }) => ({ id, mime }))))) { setValue(draft); setImages(sent); } }
    catch (error) { setValue(draft); setImages(sent); notify(error instanceof Error ? error.message : t.error); }
    finally { pending.current = false; input.current?.focus(); }
  }
  return <><form className={styles.composer} onSubmit={(event) => { event.preventDefault(); void submit(); }}
    onDragOver={(event) => { if (event.dataTransfer.types.includes("Files")) event.preventDefault(); }}
    onDrop={(event) => { if (event.dataTransfer.files.length) { event.preventDefault(); void addImages([...event.dataTransfer.files]); } }}>
    <label className="sr-only" htmlFor="chat-input">{chatText.placeholder}</label>
    <div><button type="button" aria-pressed={debate} disabled={streaming || Boolean(running)} onClick={() => { setDebate(!debate); try { localStorage.setItem("debateMode", !debate ? "1" : "0"); } catch { /* Storage unavailable. */ } }}><Scale size={16} aria-hidden="true" />{t.debate}</button>
      <button type="button" disabled={streaming || Boolean(running) || !value.trim()} onClick={() => { const draft = value; setValue(""); void startCard(draft, "plan").catch(error => { setValue(draft); notify(String(error)); }); }}><Sparkles size={16} aria-hidden="true" />{t.plan}</button></div>
    {images.length > 0 && <div className={styles.tray}>{images.map(image => <span key={image.id}>
      <img src={image.url} alt={chatText.attachments} />
      <button type="button" aria-label={chatText.removeImage} onClick={() => setImages(current => current.filter(item => item.id !== image.id))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    <div className={styles.box}>
      <input ref={picker} type="file" accept="image/png,image/jpeg,image/webp,image/gif" multiple hidden onChange={(event) => void addImages([...(event.target.files ?? [])])} />
      <details ref={attachmentMenu} className={styles.attachments} onKeyDown={(event) => {
        if (event.key === "Escape") { event.preventDefault(); event.currentTarget.open = false; event.currentTarget.querySelector("summary")?.focus(); }
      }}>
        <summary aria-label={chatText.composerActions} title={chatText.composerActions}><Plus size={22} aria-hidden="true" /></summary>
        <div className={styles.attachmentMenu}>
          <Button variant="ghost" type="button" disabled={streaming} onClick={() => {
            if (attachmentMenu.current) attachmentMenu.current.open = false;
            picker.current?.click();
          }}><ImagePlus size={18} aria-hidden="true" />{chatText.attachImage}</Button>
          <Button variant="ghost" type="button" disabled={!folder} onClick={() => {
            if (attachmentMenu.current) {
              attachmentMenu.current.open = false;
              attachmentMenu.current.querySelector("summary")?.focus();
            }
            setMemory(true);
          }}><FileEdit size={18} aria-hidden="true" />{chatText.memory}</Button>
        </div>
      </details>
      <textarea id="chat-input" ref={input} value={value} rows={1} disabled={streaming}
      placeholder={chatText.placeholder} onChange={(event) => setValue(event.target.value)}
      onPaste={(event) => { const files = [...event.clipboardData.items].flatMap(item => item.kind === "file" && item.type.startsWith("image/") ? [item.getAsFile()] : []).filter((file): file is File => file !== null); if (files.length) { event.preventDefault(); void addImages(files); } }}
      onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void submit(); } }} />
      {streaming ? <Button className={styles.sendButton} variant="danger" onClick={() => void stop()} aria-label={chatText.stop} title={chatText.stop}><Square size={18} aria-hidden="true" /></Button> :
        <Button className={styles.sendButton} variant="primary" type="submit" disabled={!canSend || Boolean(running)} aria-label={chatText.send} title={chatText.send}><ArrowUp size={20} aria-hidden="true" /></Button>}
    </div>
    <div className={styles.toolbar}><ChatToolbar config={config} /></div>
  </form>{memory && <MemoryPanel folder={folder} close={() => setMemory(false)} />}</>;
}
