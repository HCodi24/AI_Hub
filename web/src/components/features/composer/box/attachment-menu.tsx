import { useEffect, useRef, type ChangeEvent } from "react";
import { FileEdit, FileUp, ImagePlus, Plus } from "lucide-react";
import { Button } from "@/components/button/button";
import { chatText } from "@/text/chat";
import styles from "./box.module.css";

type Props = {
  disabled: boolean;
  memoryDisabled: boolean;
  onFiles: (files: File[]) => void;
  onMemory: () => void;
};

export function AttachmentMenu({ disabled, memoryDisabled, onFiles, onMemory }: Props) {
  const picker = useRef<HTMLInputElement>(null);
  const filePicker = useRef<HTMLInputElement>(null);
  const attachmentMenu = useRef<HTMLDetailsElement>(null);

  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      const menu = attachmentMenu.current;
      if (menu?.open && !menu.contains(event.target as Node)) {
        menu.open = false;
      }
    }
    document.addEventListener("pointerdown", closeOutside, true);
    return () => document.removeEventListener("pointerdown", closeOutside, true);
  }, []);

  function selectFiles(event: ChangeEvent<HTMLInputElement>) {
    const files = [...(event.target.files ?? [])];
    event.target.value = "";
    onFiles(files);
  }

  return <>
    <input ref={picker} type="file" accept="image/png,image/jpeg,image/webp,image/gif" multiple hidden onChange={selectFiles} />
    <input ref={filePicker} type="file" multiple hidden onChange={selectFiles} />
    <details ref={attachmentMenu} className={styles.attachments} onKeyDown={(event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.currentTarget.open = false;
        event.currentTarget.querySelector("summary")?.focus();
      }
    }}>
      <summary aria-label={chatText.composerActions} title={chatText.composerActions}><Plus size={22} aria-hidden="true" /></summary>
      <div data-glass="overlay" className={styles.attachmentMenu}>
        <Button variant="ghost" type="button" disabled={disabled} onClick={() => {
          if (attachmentMenu.current) attachmentMenu.current.open = false;
          picker.current?.click();
        }}><ImagePlus size={18} aria-hidden="true" />{chatText.attachImage}</Button>
        <Button variant="ghost" type="button" disabled={disabled} onClick={() => {
          if (attachmentMenu.current) attachmentMenu.current.open = false;
          filePicker.current?.click();
        }}><FileUp size={18} aria-hidden="true" />{chatText.importFile}</Button>
        <Button variant="ghost" type="button" disabled={memoryDisabled} onClick={() => {
          if (attachmentMenu.current) {
            attachmentMenu.current.open = false;
            attachmentMenu.current.querySelector("summary")?.focus();
          }
          onMemory();
        }}><FileEdit size={18} aria-hidden="true" />{chatText.memory}</Button>
      </div>
    </details>
  </>;
}
