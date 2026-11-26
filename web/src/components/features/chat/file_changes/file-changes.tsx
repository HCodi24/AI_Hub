import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { ChevronRight } from "lucide-react";
import type { ChatMessage } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { DiffPanel, parseDiff, registerDiffPanel } from "./diff-panel";
import styles from "./file-changes.module.css";

type FileEntry = ChatMessage["files"][number];

const STATUS_GLYPH: Record<NonNullable<FileEntry["status"]>, string> = { added: "+", modified: "•", deleted: "−" };

/** Small outlined marker; the glyph is decorative, the label carries the meaning. */
function StatusMarker({ status }: { status: FileEntry["status"] }) {
  const value = status || "modified";
  return <span className={styles.marker} data-status={value} role="img" aria-label={chatText.fileStatus[value]} title={chatText.fileStatus[value]}>{STATUS_GLYPH[value]}</span>;
}

function SummaryCounts({ added, removed, className }: Pick<FileEntry, "added" | "removed"> & { className: string }) {
  if (added === 0 && removed === 0) return null;
  return <span className={className}>
    {added !== 0 && <span className={styles.added}>+{added}</span>}
    {removed !== 0 && <span className={styles.removed}>−{removed}</span>}
  </span>;
}

export function FileChanges({ files, diff }: { files: FileEntry[]; diff: string }) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const originRef = useRef<HTMLButtonElement | null>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  const sections = useMemo(() => parseDiff(diff), [diff]);
  const fileGroups = useMemo(() => {
    const groups = new Map<string, { file: FileEntry; name: string }[]>();
    for (const file of files) {
      const segments = file.path.split(/[\\/]/);
      const name = segments.pop() || file.path;
      const folder = segments.join("/");
      const group = groups.get(folder);
      if (group) group.push({ file, name });
      else groups.set(folder, [{ file, name }]);
    }
    return Array.from(groups);
  }, [files]);
  const totals = useMemo(() => {
    if (files.length) return files.reduce((sum, file) => ({ added: sum.added + file.added, removed: sum.removed + file.removed }), { added: 0, removed: 0 });
    return sections.reduce((sum, section) => section.lines.reduce((inner, line) => ({
      added: inner.added + (line.kind === "add" ? 1 : 0),
      removed: inner.removed + (line.kind === "del" ? 1 : 0),
    }), sum), { added: 0, removed: 0 });
  }, [files, sections]);

  useEffect(() => {
    if (open) return registerDiffPanel(close);
  }, [open, close]);

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
          <SummaryCounts {...totals} className={styles.cardTotals} />
          <ChevronRight size={16} aria-hidden="true" className={styles.chevron} />
        </button>
        <ul className={styles.fileList}>
          {fileGroups.map(([folder, group]) => <li key={folder} className={styles.group}>
            <div className={styles.groupFolder} title={folder}>
              <bdi dir="ltr">{folder || chatText.rootFolder}</bdi>
            </div>
            <ul className={styles.fileList}>
              {group.map(({ file, name }, index) => <li key={`${index}:${file.path}`} className={styles.fileItem}>
                <button type="button" className={styles.fileRow} aria-haspopup="dialog" aria-expanded={open} title={file.path} onClick={openFrom}>
                  <StatusMarker status={file.status} />
                  <span className={styles.fileRowPath}>
                    <span className={styles.fileRowName}>{name}</span>
                  </span>
                  <SummaryCounts added={file.added} removed={file.removed} className={styles.fileRowCounts} />
                </button>
              </li>)}
            </ul>
          </li>)}
        </ul>
      </div>
      : <button ref={triggerRef} type="button" className={styles.summary} aria-haspopup="dialog" aria-expanded={open} title={chatText.openChanges} onClick={openFrom}>
        <span className={styles.summaryLabel}>{chatText.diff}</span>
        <SummaryCounts {...totals} className={styles.cardTotals} />
        <ChevronRight size={16} aria-hidden="true" className={styles.chevron} />
      </button>}
    {open && <DiffPanel files={files} diff={diff} sections={sections} onClose={close} returnFocus={originRef.current ?? triggerRef.current} />}
  </>;
}
