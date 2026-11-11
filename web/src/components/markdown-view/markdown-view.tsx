import { useEffect, useRef } from "react";
import { markdownHtml, highlight } from "@/lib/markdown";
import { chatText } from "@/text/chat";
import styles from "./markdown-view.module.css";
export function MarkdownView({ source }: { source: string }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!root.current) return;
    root.current.innerHTML = markdownHtml(source);
    for (const link of root.current.querySelectorAll("a")) {
      link.target = "_blank"; link.rel = "noopener noreferrer";
    }
    for (const code of root.current.querySelectorAll("pre > code")) {
      const language = [...code.classList].find((name) => name.startsWith("language-"))?.slice(9) || "";
      const text = code.textContent || "";
      code.innerHTML = highlight(text, language);
      const header = document.createElement("div"); header.className = styles.codeHeader;
      const label = document.createElement("span"); label.textContent = language || chatText.code;
      const button = document.createElement("button"); button.type = "button"; button.textContent = chatText.copy;
      button.addEventListener("click", async () => {
        try { await navigator.clipboard.writeText(text); button.textContent = chatText.copied; }
        catch { button.textContent = chatText.copyError; }
      });
      header.append(label, button); code.parentElement?.before(header);
    }
  }, [source]);
  return <div ref={root} className={styles.markdown} />;
}
