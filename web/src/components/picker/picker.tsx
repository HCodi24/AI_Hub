import { useRef, type ReactNode } from "react";
import { configApi } from "@/apis/config.api";

type FilePickerProps = {
  mode: "file";
  accept: string;
  onPick: (file: File) => void | Promise<void>;
  children: (open: () => void) => ReactNode;
};

type FolderPickerProps = {
  mode: "folder";
  onPick: (path: string) => void | Promise<void>;
  onOpen?: () => void;
  onError?: (error: unknown) => void;
  children: (open: () => void) => ReactNode;
};

export type PickerProps = FilePickerProps | FolderPickerProps;

/** Shared native file and folder picker. The trigger stays in the caller's markup. */
export function Picker(props: PickerProps) {
  const input = useRef<HTMLInputElement>(null);

  if (props.mode === "file") {
    const open = () => input.current?.click();
    return <>
      {props.children(open)}
      <input ref={input} type="file" accept={props.accept} hidden
        onChange={event => {
          const file = event.currentTarget.files?.[0];
          event.currentTarget.value = "";
          if (file) void props.onPick(file);
        }} />
    </>;
  }

  const open = async () => {
    props.onOpen?.();
    try {
      const path = await configApi.pickFolder();
      if (path) await props.onPick(path);
    } catch (error) {
      props.onError?.(error);
    }
  };
  return props.children(() => { void open(); });
}
