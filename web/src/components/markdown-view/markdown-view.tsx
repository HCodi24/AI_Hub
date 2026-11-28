import { useEffect, useRef } from "react";
import { markdownHtml, highlight } from "@/lib/markdown";
import { skillHighlightParts } from "@/lib/skill-tokens";
import { chatText } from "@/text/chat";
import styles from "./markdown-view.module.css";
export function MarkdownView({ source, skills }: { source: string; skills?: ReadonlySet<string> }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!root.current) return;
    root.current.innerHTML = markdownHtml(source);
    if (skills?.size) {
      const walker = document.createTreeWalker(root.current, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => node.parentElement?.closest("pre, code, a")
          ? NodeFilter.FILTER_REJECT
          : NodeFilter.FILTER_ACCEPT,
      });
      const nodes: Text[] = [];
      for (let node = walker.nextNode(); node; node = walker.nextNode()) nodes.push(node as Text);
      for (const node of nodes) {
        const parts = skillHighlightParts(node.data, skills);
        if (!parts.some((part) => part.highlighted)) continue;
        const fragment = document.createDocumentFragment();
        for (const part of parts) {
          if (!part.highlighted) {
            fragment.append(document.createTextNode(part.text));
            continue;
          }
          const pill = document.createElement("span");
          pill.className = styles.skillPill;
          pill.append(document.createTextNode(part.text));
          fragment.append(pill);
        }
        node.replaceWith(fragment);
      }
    }
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
  }, [source, skills]);
  return <div ref={root} className={styles.markdown} />;
}
