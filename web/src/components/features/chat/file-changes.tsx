import { useEffect, useId, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { createPortal } from "react-dom";
import { ChevronRight, X } from "lucide-react";
import type { ChatMessage } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import styles from "./file-changes.module.css";

type FileEntry = ChatMessage["files"][number];
type LineKind = "add" | "del" | "context" | "hunk" | "meta";
type DiffLine = { kind: LineKind; text: string; oldLine?: number; newLine?: number };
type DiffSection = { path: string; header: string[]; lines: DiffLine[] };

const HUNK = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;
const HEADER = /^(diff --git|index |new file mode|deleted file mode|old mode|new mode|similarity index|dissimilarity index|rename from|rename to|copy from|copy to|Binary files|GIT binary patch)/;

/** Only one diff panel may be open at a time across the whole message list. */
let closeActivePanel: (() => void) | null = null;

/** Splits a git diff into per-file sections and numbers unified hunk lines. */
function parseDiff(diff: string): DiffSection[] {
  const sections: DiffSection[] = [];
  if (!diff.trim()) return sections;
  const lines = diff.split("\n");
  let current: DiffSection | null = null;
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  let oldLeft = 0;
  let newLeft = 0;
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    const next = lines[index + 1] ?? "";
    const startsSection = /^diff --git /.test(line) || (!inHunk && /^--- /.test(line) && /^\+\+\+ /.test(next) && current !== null && current.lines.length > 0);
    if (startsSection) {
      current = { path: "", header: [], lines: [] };
      sections.push(current);
      inHunk = false;
    }
    if (!current) {
      current = { path: "", header: [], lines: [] };
      sections.push(current);
    }
    const hunk = HUNK.exec(line);
    if (hunk) {
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[3]);
      oldLeft = Number(hunk[2] ?? 1);
      newLeft = Number(hunk[4] ?? 1);
      inHunk = oldLeft > 0 || newLeft > 0;
      current.lines.push({ kind: "hunk", text: line });
      continue;
    }
    if (!inHunk && (/^(\+\+\+|---) /.test(line) || HEADER.test(line))) {
      current.header.push(line);
      const path = /^diff --git a\/.+? b\/(.+)$/.exec(line)?.[1] ?? /^\+\+\+ b\/(.+)$/.exec(line)?.[1] ?? /^--- a\/(.+)$/.exec(line)?.[1];
      if (path && !current.path) current.path = path;
      continue;
    }
    if (inHunk && newLeft > 0 && line.startsWith("+")) {
      current.lines.push({ kind: "add", text: line, newLine: newNo++ });
      newLeft--;
      inHunk = oldLeft > 0 || newLeft > 0;
      continue;
    }
    if (inHunk && oldLeft > 0 && line.startsWith("-")) {
      current.lines.push({ kind: "del", text: line, oldLine: oldNo++ });
      oldLeft--;
      inHunk = oldLeft > 0 || newLeft > 0;
      continue;
    }
    if (inHunk && oldLeft > 0 && newLeft > 0 && line.startsWith(" ")) {
      current.lines.push({ kind: "context", text: line, oldLine: oldNo++, newLine: newNo++ });
      oldLeft--;
      newLeft--;
      inHunk = oldLeft > 0 || newLeft > 0;
      continue;
    }
    if (line === "" && index === lines.length - 1) continue;
    current.lines.push({ kind: "meta", text: line });
  }
  return sections.filter((section) => section.header.length > 0 || section.lines.length > 0);
}

function SectionView({ section }: { section: DiffSection }) {
  return <div className={styles.section}>
    {section.header.length > 0 && <pre className={styles.meta}>{section.header.join("\n")}</pre>}
    {section.lines.length > 0 && <div className={styles.hunk}>
      {section.lines.map((line, index) => line.kind === "hunk"
        ? <div key={index} className={styles.hunkHeader}>{line.text}</div>
        : <div key={index} className={styles.row} data-kind={line.kind}>
          <span className={styles.gutter} aria-hidden="true">{line.oldLine ?? ""}</span>
          <span className={styles.gutter} aria-hidden="true">{line.newLine ?? ""}</span>
          <span className={styles.code}>{line.text || " "}</span>
        </div>)}
    </div>}
  </div>;
}

function StatusBadge({ status }: { status: FileEntry["status"] }) {
  const value = status || "modified";
  return <span className={styles.status} data-status={value} title={chatText.fileStatus[value]}>{chatText.fileStatus[value]}</span>;
}

const STATUS_GLYPH: Record<NonNullable<FileEntry["status"]>, string> = { added: "+", modified: "•", deleted: "−" };

/** Small outlined marker; the glyph is decorative, the label carries the meaning. */
function StatusMarker({ status }: { status: FileEntry["status"] }) {
  const value = status || "modified";
  return <span className={styles.marker} data-status={value} role="img" aria-label={chatText.fileStatus[value]} title={chatText.fileStatus[value]}>{STATUS_GLYPH[value]}</span>;
}

export function FileChanges({ files, diff }: { files: FileEntry[]; diff: string }) {
  const titleId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const originRef = useRef<HTMLButtonElement | null>(null);
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const sections = useMemo(() => parseDiff(diff), [diff]);
  const { entries, extras } = useMemo(() => {
    const used = new Set<number>();
    const list = files.map((file) => {
      const index = sections.findIndex((section, position) => !used.has(position) && section.path !== "" &&
        (section.path === file.path || section.path.endsWith(`/${file.path}`) || file.path.endsWith(`/${section.path}`)));
      if (index >= 0) used.add(index);
      return { file, section: index >= 0 ? sections[index] : undefined };
    });
    return { entries: list, extras: sections.filter((_, position) => !used.has(position)) };
  }, [files, sections]);
  const totals = useMemo(() => {
    if (files.length) return files.reduce((sum, file) => ({ added: sum.added + file.added, removed: sum.removed + file.removed }), { added: 0, removed: 0 });
    return sections.reduce((sum, section) => section.lines.reduce((inner, line) => ({
      added: inner.added + (line.kind === "add" ? 1 : 0),
      removed: inner.removed + (line.kind === "del" ? 1 : 0),
    }), sum), { added: 0, removed: 0 });
  }, [files, sections]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    if (closeActivePanel && closeActivePanel !== close) closeActivePanel();
    closeActivePanel = close;
    return () => { if (closeActivePanel === close) closeActivePanel = null; };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const previous = originRef.current ?? triggerRef.current;
    const node = panelRef.current;
    const focusables = () => Array.from(node?.querySelectorAll<HTMLElement>('button, [href], summary, input, select, textarea, [tabindex]:not([tabindex="-1"])') ?? [])
      .filter((element) => !element.hasAttribute("disabled") && element.getClientRects().length > 0);
    const first = focusables()[0];
    if (first) first.focus(); else node?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setOpen(false);
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusables();
      if (!items.length) {
        event.preventDefault();
        node?.focus();
        return;
      }
      const head = items[0];
      const tail = items[items.length - 1];
      const active = document.activeElement;
      const inside = node?.contains(active) ?? false;
      if (event.shiftKey && (active === head || !inside)) {
        event.preventDefault();
        tail.focus();
      } else if (!event.shiftKey && (active === tail || !inside)) {
        event.preventDefault();
        head.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("keydown", onKeyDown, true);
      queueMicrotask(() => {
        if (!closeActivePanel && previous && document.contains(previous)) previous.focus();
      });
    };
  }, [open]);

  if (!files.length && !diff.trim()) return null;
  /** Remembers which button opened the panel so focus can return to it. */
  const openFrom = (event: ReactMouseEvent<HTMLButtonElement>) => {
    originRef.current = event.currentTarget;
    setOpen(true);
  };
  return <>
    {files.length > 0
      ? <div className={styles.card}>
        <button type="button" className={styles.cardHeader} aria-haspopup="dialog" aria-expanded={open} title={chatText.openChanges} onClick={openFrom}>
          <span className={styles.cardTitle}>{chatText.edited(files.length)}</span>
          <span className={styles.cardTotals}>
            <span className={styles.added}>+{totals.added}</span>
            <span className={styles.removed}>−{totals.removed}</span>
          </span>
          <ChevronRight size={16} aria-hidden="true" className={styles.chevron} />
        </button>
        <ul className={styles.fileList}>
          {files.map((file, index) => <li key={`${index}:${file.path}`} className={styles.fileItem}>
            <button type="button" className={styles.fileRow} aria-haspopup="dialog" aria-expanded={open} title={file.path} onClick={openFrom}>
              <StatusMarker status={file.status} />
              <span className={styles.fileRowPath}>{file.path}</span>
              <span className={styles.fileRowCounts}>
                <span className={styles.added}>+{file.added}</span>
                <span className={styles.removed}>−{file.removed}</span>
              </span>
            </button>
          </li>)}
        </ul>
      </div>
      : <button ref={triggerRef} type="button" className={styles.summary} aria-haspopup="dialog" aria-expanded={open} title={chatText.openChanges} onClick={openFrom}>
        <span className={styles.summaryLabel}>{chatText.diff}</span>
        <span className={styles.added}>+{totals.added}</span>
        <span className={styles.removed}>−{totals.removed}</span>
        <ChevronRight size={16} aria-hidden="true" className={styles.chevron} />
      </button>}
    {open && createPortal(<div className={styles.layer}>
      <div className={styles.backdrop} aria-hidden="true" onClick={() => setOpen(false)} />
      <div ref={panelRef} className={styles.panel} role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}>
        <header className={styles.header}>
          <h2 id={titleId}>{chatText.changesTitle}</h2>
          <button type="button" className={styles.close} aria-label={chatText.closeChanges} onClick={() => setOpen(false)}><X size={18} aria-hidden="true" /></button>
        </header>
        <div className={styles.body}>
          {diff.trim() !== "" && sections.length === 0 && <pre className={styles.raw} aria-label={chatText.rawDiff}>{diff}</pre>}
          {entries.map(({ file, section }) => <details key={file.path} className={styles.file} open>
            <summary>
              <StatusBadge status={file.status} />
              <span className={styles.filePath}>{file.path}</span>
              <span className={styles.counts}><span className={styles.added}>+{file.added}</span><span className={styles.removed}>−{file.removed}</span></span>
            </summary>
            {section ? <SectionView section={section} /> : <p className={styles.note}>{chatText.noDiff}</p>}
          </details>)}
          {extras.map((section, index) => <details key={`${index}:${section.path}`} className={styles.file} open>
            <summary>
              <span className={styles.filePath}>{section.path || chatText.diff}</span>
            </summary>
            <SectionView section={section} />
          </details>)}
        </div>
      </div>
    </div>, document.body)}
  </>;
}
