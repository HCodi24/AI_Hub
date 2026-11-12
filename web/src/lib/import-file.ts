import { chatText } from "@/text/chat";

export const MAX_IMPORT_BYTES = 200 * 1024;
export const MAX_FOLDER_BYTES = 1024 * 1024;
export const MAX_FOLDER_FILES = 100;
export type ImportedFile = { name: string; content: string };
export type DroppedFile = File | FileSystemDirectoryEntry;
export function droppedFiles(data: DataTransfer): DroppedFile[] {
  const files: DroppedFile[] = [];
  const items = Array.from(data.items).filter(item => item.kind === "file");
  if (!items.length) return Array.from(data.files);
  for (const item of items) {
    const entry = item.webkitGetAsEntry?.();
    if (entry?.isDirectory) { files.push(entry as FileSystemDirectoryEntry); continue; }
    const file = item.getAsFile();
    if (file) files.push(file);
  }
  return files;
}

export async function importDirectory(directory: FileSystemDirectoryEntry, cancelled = () => false) {
  const files: ImportedFile[] = [];
  const ignored = new Set([".git", "node_modules", "target", ".next", "__pycache__"]);
  let bytes = 0, scanned = 0, skipped = 0, limited = false;
  async function visit(entry: FileSystemEntry, path: string, depth: number): Promise<void> {
    if (cancelled() || limited) return;
    if (++scanned > 2000 || depth > 32) { limited = true; return; }
    if (entry.isDirectory) {
      if (depth > 0 && ignored.has(entry.name)) { skipped++; return; }
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      while (!cancelled() && !limited) {
        const entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
        if (!entries.length) break;
        for (const child of entries) await visit(child, `${path}/${child.name}`, depth + 1);
      }
      return;
    }
    if (files.length >= MAX_FOLDER_FILES) { limited = true; return; }
    try {
      const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject));
      const imported = await importTextFile(file);
      const size = new TextEncoder().encode(imported.content).length;
      if (bytes + size > MAX_FOLDER_BYTES) { limited = true; return; }
      bytes += size;
      files.push({ name: path, content: imported.content });
    } catch { skipped++; }
  }
  await visit(directory, directory.name, 0);
  if (cancelled()) return null;
  if (!files.length) throw new Error(limited ? chatText.folderScanLimit : chatText.emptyFolder);
  const summary = chatText.folderSummary(files.length, skipped, limited);
  return { name: `${directory.name}/ (${files.length} file)`, content: `${summary}\n\n${importedFileText(files)}`, summary, skipped, limited };
}

export function fileReadError(error: unknown): string {
  if (error instanceof Error && ["NotFoundError", "NotReadableError", "SecurityError"].includes(error.name)) return chatText.fileUnavailable;
  return error instanceof Error ? error.message : chatText.requestError;
}

export async function importTextFile(file: Pick<File, "name" | "size" | "arrayBuffer">): Promise<ImportedFile> {
  if (file.size > MAX_IMPORT_BYTES) throw new Error(chatText.fileTooLarge);
  const bytes = new Uint8Array(await file.arrayBuffer());
  // Binary documents need their own extractors; never send their bytes as garbled text.
  if (/\.(pdf|docx?|xlsx?|pptx?|zip|rar|7z|exe|dll|wasm)$/i.test(file.name)) throw new Error(chatText.unsupportedFile);
  let content: string;
  try {
    const encoding = bytes[0] === 0xff && bytes[1] === 0xfe ? "utf-16le" : bytes[0] === 0xfe && bytes[1] === 0xff ? "utf-16be" : "utf-8";
    content = new TextDecoder(encoding, { fatal: true }).decode(bytes);
  } catch { throw new Error(chatText.unsupportedFile); }
  if (/[\u0000-\u0008\u000e-\u001f\u007f]/.test(content) || content.startsWith("%PDF-")) throw new Error(chatText.unsupportedFile);
  if (!content.trim()) throw new Error(chatText.emptyFile);
  return { name: file.name, content };
}

export function importedFileText(files: ImportedFile[]): string {
  return files.map(file => {
    const longest = (file.content.match(/`+/g) || []).reduce((max, run) => Math.max(max, run.length), 2);
    const fence = "`".repeat(longest + 1);
    return `Attached file ${JSON.stringify(file.name)}:\n${fence}\n${file.content}\n${fence}`;
  }).join("\n\n");
}

// Existing chats store imported files in the prompt; only split a complete attachment suffix for display.
export function splitImportedFileText(content: string): { text: string; files: ImportedFile[] } {
  const unchanged = { text: content, files: [] };
  const blocks = [...content.matchAll(/^Attached file ("(?:[^"\\\r\n]|\\.)*"):\r?\n(`{3,})\r?\n([\s\S]*?)\r?\n\2(?=\r?$)/gm)];
  if (!blocks.length) return unchanged;
  const start = blocks[0].index!;
  if (start > 0 && !/\r?\n\r?\n$/.test(content.slice(0, start))) return unchanged;
  const files: ImportedFile[] = [];
  let end = start;
  for (const block of blocks) {
    if (content.slice(end, block.index).trim()) return unchanged;
    try { files.push({ name: JSON.parse(block[1]) as string, content: block[3] }); }
    catch { return unchanged; }
    end = block.index! + block[0].length;
  }
  if (content.slice(end).trim()) return unchanged;
  return { text: content.slice(0, start).trimEnd(), files };
}
