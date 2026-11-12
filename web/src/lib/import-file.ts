import { chatText } from "@/text/chat";

export const MAX_IMPORT_BYTES = 200 * 1024;
export type ImportedFile = { name: string; content: string };
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
