import { useEffect, type RefObject } from "react";
import { useShallow } from "zustand/react/shallow";
import { useChatStore } from "@/components/features/chat/chat-store";
import { PermissionMenu } from "./permission-menu";

type Props = {
  supportsCommands: boolean;
  menuRef: RefObject<HTMLDetailsElement | null>;
  otherMenuRef: RefObject<HTMLDetailsElement | null>;
};

/** The shared Claude/Codex permission control, including chat state and menu interaction. */
export function ChatPermission({ supportsCommands, menuRef, otherMenuRef }: Props) {
  const { permission, streaming, configure } = useChatStore(useShallow(state => ({
    permission: state.settings.permission, streaming: state.streaming, configure: state.configure,
  })));
  useEffect(() => {
    function closeOutside(event: PointerEvent) {
      if (menuRef.current?.open && !menuRef.current.contains(event.target as Node)) {
        menuRef.current.open = false;
      }
    }
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape" || !menuRef.current?.open) return;
      event.preventDefault();
      menuRef.current.open = false;
      menuRef.current.querySelector("summary")?.focus();
    }
    document.addEventListener("pointerdown", closeOutside, true);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside, true);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuRef]);
  return <PermissionMenu value={permission} disabled={streaming} cli={supportsCommands} menuRef={menuRef}
    onOpen={() => { if (otherMenuRef.current) otherMenuRef.current.open = false; }}
    onPick={permission => configure({ permission })} />;
}
