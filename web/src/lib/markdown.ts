import { marked } from "marked";
import DOMPurify from "dompurify";
import hljs from "highlight.js";
export function hideMemory(source: string): string {
  return source.replace(/<memory\b[^>]*>[\s\S]*?(?:<\/memory\s*>|$)/gi, "");
}
export function markdownHtml(source: string): string {
  const html = marked.parse(hideMemory(source), { async: false, gfm: true, breaks: true });
  return DOMPurify.sanitize(html, { FORBID_TAGS: ["style", "input", "form", "iframe"], FORBID_ATTR: ["style"] });
}
export function highlight(code: string, language: string): string {
  if (hljs.getLanguage(language)) return hljs.highlight(code, { language }).value;
  return code.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
