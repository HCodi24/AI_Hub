import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { z } from "zod";
import { ArrowUp, Square, Scale, Sparkles, ImagePlus, Plus, X, FileEdit, FileUp } from "lucide-react";
import { droppedFiles, fileReadError, importDirectory, importTextFile, importedFileText, type DroppedFile, type ImportedFile } from "@/lib/import-file";
import { Button } from "@/components/button/button";
import { http } from "@/lib/http";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
import { startCard } from "@/components/features/chat/card-actions";
import { notify } from "@/components/toast/toast";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { ChatToolbar } from "./chat-toolbar";
import { AttachmentCard } from "@/components/features/chat/attachment-card";
import { MemoryPanel } from "@/components/features/memory/memory-panel";
import type { Config } from "@/schema/config.schema";
import styles from "./composer.module.css";
const MAX_FILES = 5;
const UploadSchema = z.object({ id: z.string(), mime: z.string(), url: z.string() });
type Image = z.infer<typeof UploadSchema>;
type Attachment = (Image & { kind: "image"; name: string }) | (ImportedFile & { kind: "text"; id: string });
const readBase64 = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = () => reject(reader.error);
  reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
  reader.readAsDataURL(file);
});
export function Composer({ dropTarget, config, streaming, send, stop }: { dropTarget: RefObject<HTMLElement | null>; config?: Config; streaming: boolean; send: (text: string, direct?: boolean, attachments?: { id: string; mime: string }[]) => Promise<boolean>; stop: () => Promise<void> }) {
  const [debate, setDebate] = useState(false);
  const running = useCardStore(s => s.running);
  const chatId = useChatStore(s => s.id);
  const folder = useChatStore(s => s.settings.folder);
  const [memory, setMemory] = useState(false);
  useEffect(() => { try { setDebate(["1", "true"].includes(localStorage.getItem("debateMode") || "")); } catch { /* Storage unavailable. */ } }, []);
  const [value, setValue] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const attached = useRef<Attachment[]>([]);
  const importing = useRef(false);
  const [uploading, setUploading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const updateAttachments = useCallback((next: Attachment[]) => { attached.current = next; setAttachments(next); }, []);
  const [recalled, setRecalled] = useState<{ label: string; text: string }[]>([]);
  const input = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const filePicker = useRef<HTMLInputElement>(null);
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
  useEffect(() => { if (owner.current !== chatId) { owner.current = chatId; attached.current = []; setAttachments([]); setDragging(false); } }, [chatId]);
  useEffect(() => {
    if (input.current) { input.current.style.height = "auto"; input.current.style.height = `${Math.min(input.current.scrollHeight, 180)}px`; }
  }, [value]);
  const addFiles = useCallback(async (files: DroppedFile[]) => {
    if (importing.current || pending.current || useChatStore.getState().streaming || useCardStore.getState().running || !files.length) return;
    importing.current = true; setUploading(true);
    let id = useChatStore.getState().id;
    if (!id) { id = crypto.randomUUID(); owner.current = id; useChatStore.setState({ id }); }
    try {
      for (const file of files) {
        if (useChatStore.getState().id !== id) break;
        if (attached.current.length >= MAX_FILES) { notify(chatText.maxFiles); break; }
        try {
          let attachment: Attachment;
          if ("isDirectory" in file) {
            const imported = await importDirectory(file, () => useChatStore.getState().id !== id);
            if (!imported) break;
            attachment = { ...imported, id: crypto.randomUUID(), kind: "text" };
          } else if (file.type.startsWith("image/") || /\.(png|jpe?g|webp|gif)$/i.test(file.name)) {
            if (file.size > 10 * 1024 * 1024) throw new Error(chatText.imageTooLarge);
            const data = await readBase64(file);
            const response = await http.request("/attachments", { method: "POST", body: JSON.stringify({ chat_id: id, data }) });
            const image = UploadSchema.parse(await response.json());
            attachment = { ...image, kind: "image", name: file.name, url: `data:${image.mime};base64,${data}` };
          } else {
            attachment = { ...await importTextFile(file), id: crypto.randomUUID(), kind: "text" };
          }
          if (useChatStore.getState().id === id) updateAttachments([...attached.current, attachment]);
        } catch (error) { notify(`${file.name}: ${fileReadError(error)}`); }
      }
    } finally {
      importing.current = false; setUploading(false);
      if (picker.current) picker.current.value = "";
      if (filePicker.current) filePicker.current.value = "";
    }
  }, [updateAttachments]);
  useEffect(() => {
    const target = dropTarget.current;
    if (!target) return;
    const busy = () => importing.current || pending.current || useChatStore.getState().streaming || Boolean(useCardStore.getState().running);
    const over = (event: DragEvent) => {
      if (!event.dataTransfer?.types.includes("Files")) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = busy() ? "none" : "copy";
      setDragging(!busy());
    };
    const leave = (event: DragEvent) => {
      if (!(event.relatedTarget instanceof Node) || !target.contains(event.relatedTarget)) setDragging(false);
    };
    const drop = (event: DragEvent) => {
      if (!event.dataTransfer?.types.includes("Files")) return;
      event.preventDefault(); setDragging(false);
      if (busy()) return;
      void addFiles(droppedFiles(event.dataTransfer));
    };
    target.addEventListener("dragover", over);
    target.addEventListener("dragleave", leave);
    target.addEventListener("drop", drop);
    return () => {
      target.removeEventListener("dragover", over);
      target.removeEventListener("dragleave", leave);
      target.removeEventListener("drop", drop);
    };
  }, [addFiles, dropTarget]);
  const canSend = Boolean(value.trim()) || attachments.length > 0;
  async function submit() {
    if (pending.current || importing.current || streaming || Boolean(running) || !canSend) return;
    pending.current = true;
    const origin = useChatStore.getState().id;
    const draft = value, sent = attached.current, picked = recalled; setValue(""); updateAttachments([]); setRecalled([]);
    // Picked memory goes in front of the message, so the AI reads it without opening files (an extra model call).
    const prompt = picked.length
      ? `Recalled memory (picked by the user):\n${picked.map(item => `[${item.label}]\n${item.text}`).join("\n\n")}\n\n${draft.trim()}`
      : draft.trim();
    const text = [prompt, importedFileText(sent.filter((file): file is Extract<Attachment, { kind: "text" }> => file.kind === "text"))].filter(Boolean).join("\n\n");
    const restore = () => { if (useChatStore.getState().id === origin) { setValue(draft); updateAttachments(sent); setRecalled(picked); } };
    try { if (!(await send(text, false, sent.filter((file): file is Extract<Attachment, { kind: "image" }> => file.kind === "image").map(({ id, mime }) => ({ id, mime }))))) restore(); }
    catch (error) { restore(); notify(error instanceof Error ? error.message : t.error); }
    finally { pending.current = false; input.current?.focus(); }
  }
  return <><form className={styles.composer} data-dragging={dragging || undefined} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
    <label className="sr-only" htmlFor="chat-input">{chatText.placeholder}</label>
    <div><button type="button" aria-pressed={debate} disabled={streaming || Boolean(running)} onClick={() => { setDebate(!debate); try { localStorage.setItem("debateMode", !debate ? "1" : "0"); } catch { /* Storage unavailable. */ } }}><Scale size={16} aria-hidden="true" />{t.debate}</button>
      <button type="button" disabled={streaming || uploading || attachments.length > 0 || Boolean(running) || !value.trim()} onClick={() => { const draft = value; setValue(""); void startCard(draft, "plan").catch(error => { setValue(draft); notify(String(error)); }); }}><Sparkles size={16} aria-hidden="true" />{t.plan}</button></div>
    {recalled.length > 0 && <div className={styles.recalled} aria-label={t.recalled}>{recalled.map((item, index) => <span key={item.label}>
      <FileEdit size={14} aria-hidden="true" /><span title={item.label}>{item.label}</span>
      <button type="button" aria-label={`${t.removeMemory}: ${item.label}`} onClick={() => setRecalled(current => current.filter((_, i) => i !== index))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    {attachments.length > 0 && <div className={styles.tray} aria-label={chatText.attachedFiles}>{attachments.map(file => file.kind === "text"
      ? <AttachmentCard key={file.id} name={file.name} disabled={uploading} remove={() => updateAttachments(attached.current.filter(item => item.id !== file.id))} />
      : <span key={file.id} title={file.name}><img src={file.url} alt={file.name || chatText.attachments} />
        <button type="button" disabled={uploading} aria-label={`${chatText.removeFile}: ${file.name}`} onClick={() => updateAttachments(attached.current.filter(item => item.id !== file.id))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    <div className={styles.box}>
      <input ref={picker} type="file" accept="image/png,image/jpeg,image/webp,image/gif" multiple hidden onChange={(event) => void addFiles([...(event.target.files ?? [])])} />
      <input ref={filePicker} type="file" multiple hidden onChange={(event) => void addFiles([...(event.target.files ?? [])])} />
      <details ref={attachmentMenu} className={styles.attachments} onKeyDown={(event) => {
        if (event.key === "Escape") { event.preventDefault(); event.currentTarget.open = false; event.currentTarget.querySelector("summary")?.focus(); }
      }}>
        <summary aria-label={chatText.composerActions} title={chatText.composerActions}><Plus size={22} aria-hidden="true" /></summary>
        <div className={styles.attachmentMenu}>
          <Button variant="ghost" type="button" disabled={streaming || uploading || Boolean(running)} onClick={() => {
            if (attachmentMenu.current) attachmentMenu.current.open = false;
            picker.current?.click();
          }}><ImagePlus size={18} aria-hidden="true" />{chatText.attachImage}</Button>
          <Button variant="ghost" type="button" disabled={streaming || uploading || Boolean(running)} onClick={() => {
            if (attachmentMenu.current) attachmentMenu.current.open = false;
            filePicker.current?.click();
          }}><FileUp size={18} aria-hidden="true" />{chatText.importFile}</Button>
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
      onPaste={(event) => { const files = [...event.clipboardData.items].flatMap(item => item.kind === "file" ? [item.getAsFile()] : []).filter((file): file is File => file !== null); if (files.length) { event.preventDefault(); void addFiles(files); } }}
      onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void submit(); } }} />
      {streaming ? <Button className={styles.sendButton} variant="danger" onClick={() => void stop()} aria-label={chatText.stop} title={chatText.stop}><Square size={18} aria-hidden="true" /></Button> :
        <Button className={styles.sendButton} variant="primary" type="submit" disabled={uploading || !canSend || Boolean(running)} aria-label={chatText.send} title={chatText.send}><ArrowUp size={20} aria-hidden="true" /></Button>}
    </div>
    <div className={styles.toolbar}><ChatToolbar config={config} /></div>
    {(uploading || dragging) && <p className={styles.fileHint} role="status">{uploading ? chatText.importingFiles : chatText.dropFiles}</p>}
  </form>{memory && <MemoryPanel folder={folder} close={() => setMemory(false)}
    attach={(label, text) => setRecalled(current => [...current.filter(item => item.label !== label), { label, text }])} />}</>;
}
