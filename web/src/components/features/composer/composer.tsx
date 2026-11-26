import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { Scale, X, FileEdit, MousePointerClick } from "lucide-react";
import { addonsApi } from "@/apis/addons.api";
import { droppedFiles, fileReadError, importDirectory, importTextFile, importedFileText, type DroppedFile, type ImportedFile } from "@/lib/import-file";
import { http } from "@/lib/http";
import { chatText } from "@/text/chat";
import { previewText } from "@/text/preview";
import { cardText as t } from "@/text/cards";
import { notify } from "@/components/toast/toast";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { ChatToolbar } from "./toolbar/chat-toolbar";
import { AttachmentMenu } from "./box/attachment-menu";
import { SendButton } from "./box/send-button";
import { SkillSuggestions } from "./skill-suggestions";
import { AttachmentCard } from "@/components/features/chat/attachment-card";
import { previewSelectionText } from "@/lib/preview-selection";
import { MemoryPanel } from "@/components/features/memory/memory-panel";
import type { Config } from "@/schema/config.schema";
import styles from "./composer.module.css";
const MAX_FILES = 5;
// Match the whitespace used by Rust's trim_start and char::is_whitespace.
const FIRST_SLASH_TOKEN = new RegExp("^(\\p{White_Space}*)/([^\\p{White_Space}]*)", "u");
const UNICODE_WHITESPACE = /^\p{White_Space}$/u;
type SuggestionToken = { start: number; end: number; prefix: string };
type HighlightPart = { text: string; highlighted: boolean };
function suggestionTokenAt(value: string, caret: number): SuggestionToken | null {
  let start = caret;
  while (start > 0 && !UNICODE_WHITESPACE.test(value[start - 1])) start--;
  if (value[start] !== "/" || caret <= start || caret > value.length) return null;
  let end = start + 1;
  while (end < value.length && !UNICODE_WHITESPACE.test(value[end])) end++;
  const token = value.slice(start + 1, end);
  // A second separator indicates a URL or path rather than a skill command.
  if (token.includes("/") || token.includes("\\")) return null;
  return { start, end, prefix: value.slice(start + 1, caret).toLowerCase() };
}
function skillHighlightParts(value: string, knownSkills: ReadonlySet<string>): HighlightPart[] {
  const parts: HighlightPart[] = [];
  const tokenPattern = /\/([^\p{White_Space}]*)/gu;
  let cursor = 0;
  for (const match of value.matchAll(tokenPattern)) {
    const start = match.index;
    const token = match[1];
    const end = start + match[0].length;
    const hasBoundary = start === 0 || UNICODE_WHITESPACE.test(value[start - 1]);
    if (!hasBoundary || token.includes("/") || token.includes("\\") || !knownSkills.has(token)) continue;
    if (cursor < start) parts.push({ text: value.slice(cursor, start), highlighted: false });
    parts.push({ text: match[0], highlighted: true });
    cursor = end;
  }
  if (cursor < value.length || parts.length === 0) parts.push({ text: value.slice(cursor), highlighted: false });
  return parts;
}
function syncHighlight(textarea: HTMLTextAreaElement | null, highlight: HTMLDivElement | null) {
  if (!textarea || !highlight) return;
  highlight.style.right = `${textarea.offsetWidth - textarea.clientWidth}px`;
  highlight.scrollTop = textarea.scrollTop;
  highlight.scrollLeft = textarea.scrollLeft;
}
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
  const queue = useChatStore(s => s.queue);
  const unqueue = useChatStore(s => s.unqueue);
  const [memory, setMemory] = useState(false);
  useEffect(() => { try { setDebate(["1", "true"].includes(localStorage.getItem("debateMode") || "")); } catch { /* Storage unavailable. */ } }, []);
  const [value, setValue] = useState("");
  const [previewSelections, setPreviewSelections] = useState<{ id: string; label: string; text: string }[]>([]);
  useEffect(() => {
    const attach = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const { label, text } = event.detail ?? {};
      if (typeof label !== "string" || typeof text !== "string" || !text) return;
      const item = { id: crypto.randomUUID(), label: label || previewText.selectedElements, text };
      setPreviewSelections(current => [...current, item]);
      input.current?.focus();
    };
    window.addEventListener("mutis-preview-selection", attach);
    return () => window.removeEventListener("mutis-preview-selection", attach);
  }, []);
  const [focused, setFocused] = useState(false);
  const [composing, setComposing] = useState(false);
  const [selection, setSelection] = useState({ start: 0, end: 0 });
  const [suggestionsDismissed, setSuggestionsDismissed] = useState(false);
  const [activeSkill, setActiveSkill] = useState<string | null>(null);
  const skillListId = useId();
  const addons = useQuery({ queryKey: ["addons"], queryFn: ({ signal }) => addonsApi.overview(signal) });
  const { shared_skills: sharedSkills, builtin_skills: builtinSkills } =
    (addons.isSuccess ? addons.data : undefined) ?? {};
  const knownSkills = useMemo(
    () => new Set<string>([...(sharedSkills ?? []), ...(builtinSkills ?? [])]),
    [sharedSkills, builtinSkills],
  );
  const highlightParts = useMemo(() => skillHighlightParts(value, knownSkills), [value, knownSkills]);
  const firstSlashToken = FIRST_SLASH_TOKEN.exec(value);
  const suggestionToken = selection.start === selection.end ? suggestionTokenAt(value, selection.start) : null;
  const prefix = focused && !streaming && !composing && !suggestionsDismissed && suggestionToken
    ? suggestionToken.prefix
    : null;
  const suggestions = useMemo(() => {
    if (prefix === null) return [];
    return [...knownSkills].filter(name => name.toLowerCase().startsWith(prefix));
  }, [prefix, knownSkills]);
  const menuOpen = suggestions.length > 0;
  const activeIndex = Math.max(0, suggestions.indexOf(activeSkill ?? ""));
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const attached = useRef<Attachment[]>([]);
  const importing = useRef(false);
  const [uploading, setUploading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const updateAttachments = useCallback((next: Attachment[]) => { attached.current = next; setAttachments(next); }, []);
  const [recalled, setRecalled] = useState<{ label: string; text: string }[]>([]);
  const input = useRef<HTMLTextAreaElement>(null);
  const highlight = useRef<HTMLDivElement>(null);
  const nextCaret = useRef<number | null>(null);
  const pending = useRef(false);
  // Uploads are stored per chat id, so images picked for another chat no longer apply.
  const owner = useRef(chatId);
  useEffect(() => { if (owner.current !== chatId) { owner.current = chatId; attached.current = []; setAttachments([]); setPreviewSelections([]); setDragging(false); } }, [chatId]);
  useEffect(() => {
    if (input.current) {
      input.current.style.height = "auto";
      input.current.style.height = `${Math.min(input.current.scrollHeight, 180)}px`;
      syncHighlight(input.current, highlight.current);
    }
  }, [value]);
  useEffect(() => {
    const textarea = input.current;
    if (!textarea) return;
    const sync = () => syncHighlight(textarea, highlight.current);
    sync();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", sync);
      return () => window.removeEventListener("resize", sync);
    }
    const observer = new ResizeObserver(sync);
    observer.observe(textarea);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const caret = nextCaret.current;
    if (caret === null) return;
    input.current?.setSelectionRange(caret, caret);
    nextCaret.current = null;
  }, [value, selection]);

  function trackSelection(textarea: HTMLTextAreaElement) {
    const { selectionStart: start, selectionEnd: end } = textarea;
    setSelection(current => current.start === start && current.end === end ? current : { start, end });
  }

  function selectSkill(name: string) {
    if (!suggestionToken) return;
    const before = `${value.slice(0, suggestionToken.start)}/${name}`;
    const appendSpace = suggestionToken.end === value.length;
    const after = value.slice(suggestionToken.end) + (appendSpace ? " " : "");
    const caret = before.length + (appendSpace ? 1 : 0);
    nextCaret.current = caret;
    setValue(before + after);
    setSelection({ start: caret, end: caret });
    setSuggestionsDismissed(true);
  }

  function handleInputKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.shiftKey || composing || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
    if (event.key === "Enter" && event.repeat) {
      event.preventDefault();
      return;
    }
    if (menuOpen) {
      switch (event.key) {
        case "ArrowDown":
        case "ArrowUp": {
          event.preventDefault();
          const step = event.key === "ArrowDown" ? 1 : -1;
          setActiveSkill(suggestions[(activeIndex + step + suggestions.length) % suggestions.length]);
          return;
        }
        case "Enter":
          event.preventDefault();
          selectSkill(suggestions[activeIndex]);
          return;
        case "Escape":
          event.preventDefault();
          setSuggestionsDismissed(true);
          return;
      }
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void submit();
    }
  }
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
  const canSend = Boolean(value.trim()) || attachments.length > 0 || previewSelections.length > 0;
  async function submit() {
    if (pending.current || importing.current || Boolean(running) || !canSend) return;
    pending.current = true;
    const origin = useChatStore.getState().id;
    const draft = value, sent = attached.current, picked = recalled; setValue(""); updateAttachments([]); setRecalled([]);
    const sentSelections = previewSelections;
    setPreviewSelections([]);
    let prompt = draft.trim();
    if (picked.length) {
      const context = `Recalled memory (picked by the user):\n${picked.map(item => `[${item.label}]\n${item.text}`).join("\n\n")}`;
      // The backend resolves slash commands from the first token, even if suggestions are unavailable.
      prompt = firstSlashToken?.[2]
        ? `${prompt}\n\n${context}`
        : `${context}\n\n${prompt}`;
    }
    const text = [prompt, previewSelectionText(sentSelections), importedFileText(sent.filter((file): file is Extract<Attachment, { kind: "text" }> => file.kind === "text"))].filter(Boolean).join("\n\n");
    const restore = () => { if (useChatStore.getState().id === origin) {
      setValue(draft); updateAttachments(sent); setRecalled(picked);
      setPreviewSelections(current => [...sentSelections, ...current]);
    } };
    try { if (!(await send(text, false, sent.filter((file): file is Extract<Attachment, { kind: "image" }> => file.kind === "image").map(({ id, mime }) => ({ id, mime }))))) restore(); }
    catch (error) { restore(); notify(error instanceof Error ? error.message : t.error); }
    finally { pending.current = false; input.current?.focus(); }
  }
  return <><form data-glass="composer" className={styles.composer} data-dragging={dragging || undefined} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
    <label className="sr-only" htmlFor="chat-input">{chatText.placeholder}</label>
    {queue.length > 0 && <div className={styles.queue} aria-label={t.queued}>{queue.map((item, index) => <span key={`${index}-${item.text}`}>
      <span title={item.text}>{item.text}</span>
      <button type="button" aria-label={`${t.removeQueued}: ${item.text}`} onClick={() => unqueue(index)}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    {recalled.length > 0 && <div className={styles.recalled} aria-label={t.recalled}>{recalled.map((item, index) => <span key={item.label}>
      <FileEdit size={14} aria-hidden="true" /><span title={item.label}>{item.label}</span>
      <button type="button" aria-label={`${t.removeMemory}: ${item.label}`} onClick={() => setRecalled(current => current.filter((_, i) => i !== index))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    {previewSelections.length > 0 && <div className={styles.recalled} aria-label={previewText.selectedElements}>{previewSelections.map(item => <span key={item.id}>
      <MousePointerClick size={14} aria-hidden="true" /><span className={styles.previewSelector} title={item.label}>{item.label.split(" > ").pop()}</span>
      <button type="button" aria-label={`${previewText.removeElement}: ${item.label}`} onClick={() => setPreviewSelections(current => current.filter(selection => selection.id !== item.id))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    {attachments.length > 0 && <div className={styles.tray} aria-label={chatText.attachedFiles}>{attachments.map(file => file.kind === "text"
      ? <AttachmentCard key={file.id} name={file.name} disabled={uploading} remove={() => updateAttachments(attached.current.filter(item => item.id !== file.id))} />
      : <span key={file.id} title={file.name}><img src={file.url} alt={file.name || chatText.attachments} />
        <button type="button" disabled={uploading} aria-label={`${chatText.removeFile}: ${file.name}`} onClick={() => updateAttachments(attached.current.filter(item => item.id !== file.id))}><X size={12} aria-hidden="true" /></button></span>)}</div>}
    {menuOpen && <SkillSuggestions id={skillListId} names={suggestions} activeIndex={activeIndex} onSelect={selectSkill} />}
    <div data-glass="composer-box" className={styles.box}>
      <AttachmentMenu
        disabled={streaming || uploading || Boolean(running)}
        memoryDisabled={!folder}
        onFiles={(files) => void addFiles(files)}
        onMemory={() => setMemory(true)}
      />
      <div className={styles.inputWrap}>
      <div ref={highlight} className={styles.inputHighlight} aria-hidden="true">
        {highlightParts.map((part, index) => part.highlighted
          ? <mark key={index}>{part.text}</mark>
          : <span key={index}>{part.text}</span>)}
        {value.endsWith("\n") && "\u200b"}
      </div>
      <textarea id="chat-input" ref={input} value={value} rows={1}
      placeholder={chatText.placeholder}
      aria-autocomplete="list" aria-controls={menuOpen ? skillListId : undefined}
      aria-activedescendant={menuOpen ? `${skillListId}-${activeIndex}` : undefined}
      aria-describedby={menuOpen ? `${skillListId}-hint` : undefined}
      onFocus={(event) => {
        setFocused(true);
        setSuggestionsDismissed(false);
        setActiveSkill(null);
        trackSelection(event.currentTarget);
      }}
      onBlur={() => { setFocused(false); setComposing(false); }}
      onSelect={(event) => trackSelection(event.currentTarget)}
      onChange={(event) => {
        setValue(event.currentTarget.value);
        trackSelection(event.currentTarget);
        setSuggestionsDismissed(false);
        setActiveSkill(null);
      }}
      onCompositionStart={() => setComposing(true)}
      onCompositionEnd={(event) => {
        setComposing(false);
        trackSelection(event.currentTarget);
      }}
      onPaste={(event) => { const files = [...event.clipboardData.items].flatMap(item => item.kind === "file" ? [item.getAsFile()] : []).filter((file): file is File => file !== null); if (files.length) { event.preventDefault(); void addFiles(files); } }}
      onKeyDown={handleInputKeyDown}
      onScroll={(event) => {
        syncHighlight(event.currentTarget, highlight.current);
      }} />
      </div>
      <SendButton
        streaming={streaming}
        disabled={uploading || !canSend || Boolean(running)}
        onStop={() => void stop()}
      />
    </div>
    <div data-glass="toolbar" className={styles.toolbar}>
      <ChatToolbar config={config} />
      <button data-glass="debate-button" className={styles.debateButton} type="button" aria-label={t.debate} title={t.debate} aria-pressed={debate} disabled={streaming || Boolean(running)} onClick={() => { setDebate(!debate); try { localStorage.setItem("debateMode", !debate ? "1" : "0"); } catch { /* Storage unavailable. */ } }}><Scale size={16} aria-hidden="true" /><span>{t.debate}</span></button>
    </div>
    {(uploading || dragging) && <p className={styles.fileHint} role="status">{uploading ? chatText.importingFiles : chatText.dropFiles}</p>}
  </form>{memory && <MemoryPanel folder={folder} close={() => setMemory(false)}
    attach={(label, text) => setRecalled(current => [...current.filter(item => item.label !== label), { label, text }])} />}</>;
}
