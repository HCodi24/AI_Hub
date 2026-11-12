import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, ChevronDown, Folder, FolderOpen, MessagesSquare } from "lucide-react";
import { chatApi } from "@/apis/chat.api";
import { configApi } from "@/apis/config.api";
import { useChatStore } from "@/components/features/chat/chat-store";
import { notify } from "@/components/toast/toast";
import { chatText } from "@/text/chat";
import { folderName } from "@/lib/utils";
import styles from "./empty-state.module.css";
const MAX_PROJECTS = 5;
/** New-chat screen: one compact picker for the project (working folder) the chat will run in. */
export function EmptyState() {
  const folder = useChatStore(s => s.settings.folder);
  const configure = useChatStore(s => s.configure);
  const chats = useQuery({ queryKey: ["chats"], queryFn: ({ signal }) => chatApi.list(signal) });
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent | KeyboardEvent) => {
      if (event instanceof KeyboardEvent ? event.key === "Escape" : !root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", close);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", close); };
  }, [open]);
  // Chats arrive newest first, so recently used projects come first; the selected one is always listed.
  const projects = [...new Set([folder, ...(chats.data || []).map(chat => chat.folder), config.data?.default_folder || ""])]
    .filter(Boolean).slice(0, MAX_PROJECTS);
  async function openFolder() {
    setOpen(false);
    try { const path = await configApi.pickFolder(); if (path) configure({ folder: path }); }
    catch { notify(chatText.pickFolderFailed); }
  }
  return <div className={styles.empty}><MessagesSquare size={36} aria-hidden="true" /><h1>{chatText.greeting}</h1><p>{chatText.intro}</p>
    <div ref={root} className={styles.picker}>
      <button className={styles.trigger} aria-haspopup="listbox" aria-expanded={open} title={folder} onClick={() => setOpen(!open)}>
        {folder ? <Folder size={18} aria-hidden="true" /> : <MessagesSquare size={18} aria-hidden="true" />}<span>{folder ? folderName(folder) : chatText.noFolder}</span><ChevronDown size={16} aria-hidden="true" /></button>
      {open && <div className={styles.menu} role="listbox" aria-label={chatText.folder}>
        <button role="option" aria-selected={!folder} title={chatText.normalChatHint} onClick={() => { configure({ folder: "" }); setOpen(false); }}>
          <MessagesSquare size={18} aria-hidden="true" /><span>{chatText.noFolder}</span>{!folder && <Check size={18} aria-hidden="true" />}</button>
        {projects.map(path => <button key={path} role="option" aria-selected={path === folder} title={path} onClick={() => { configure({ folder: path }); setOpen(false); }}>
          <Folder size={18} aria-hidden="true" /><span>{folderName(path)}</span>{path === folder && <Check size={18} aria-hidden="true" />}</button>)}
        <button className={styles.openFolder} onClick={() => void openFolder()}><FolderOpen size={18} aria-hidden="true" /><span>{chatText.openFolder}</span></button>
      </div>}
    </div></div>;
}
