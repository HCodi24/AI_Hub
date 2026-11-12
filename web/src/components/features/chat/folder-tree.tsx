import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight, File, Folder } from "lucide-react";
import { Button } from "@/components/button/button";
import { configApi } from "@/apis/config.api";
import { chatText } from "@/text/chat";
import { folderName } from "@/lib/utils";
import { ApiError } from "@/lib/http";
import styles from "./folder-tree.module.css";

type Entry = { name: string; dir: boolean };
const SEP = (p: string) => (p.endsWith("\\") || p.endsWith("/") ? p : p + "\\");
const loadError = (error: unknown) => error instanceof ApiError && error.status === 404
  ? chatText.treeBackendMissing : chatText.treeLoadFailed;

function Node({ path, entry, depth }: { path: string; entry: Entry; depth: number }) {
  const [kids, setKids] = useState<Entry[] | null>(null);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const full = SEP(path) + entry.name;
  const pad = { paddingLeft: depth * 16 + 8 };
  if (!entry.dir) return <div className={styles.row} style={pad} title={entry.name}><span className={styles.spacer} /><File size={16} aria-hidden="true" /><span className={styles.name}>{entry.name}</span></div>;
  async function toggle() {
    if (loading) return;
    if (open && !error) { setOpen(false); return; }
    setOpen(true);
    if (kids) return;
    setError(""); setLoading(true);
    try { setKids(await configApi.listDir(full)); }
    catch (error) { setError(loadError(error)); }
    finally { setLoading(false); }
  }
  return <div>
    <button className={styles.row} style={pad} title={entry.name} aria-expanded={open} onClick={() => void toggle()}>
      {open ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}<Folder size={16} aria-hidden="true" /><span className={styles.name}>{entry.name}</span></button>
    {open && <div style={{ paddingLeft: (depth + 1) * 16 }}>
      {loading && <span role="status">{chatText.loading}</span>}
      {error && <div role="alert">{error} <Button size="sm" onClick={() => void toggle()}>{chatText.retry}</Button></div>}
      {kids?.length === 0 && <span>{chatText.treeEmpty}</span>}
    </div>}
    {open && kids?.map(k => <Node key={k.name} path={full} entry={k} depth={depth + 1} />)}
  </div>;
}

export function FolderTree({ folder }: { folder: string }) {
  const [root, setRoot] = useState<Entry[] | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setRoot(null); setError("");
    configApi.listDir(folder).then(entries => { if (active) setRoot(entries); })
      .catch(error => { if (active) setError(loadError(error)); });
    return () => { active = false; };
  }, [folder, attempt]);
  return <>
    <h2>{chatText.projectTree}</h2>
    <div className={styles.project} title={folder}><Folder size={16} aria-hidden="true" /><strong className={styles.name}>{folderName(folder)}</strong></div>
    <div className={styles.entries}>
      {error ? <div role="alert">{error} <Button size="sm" onClick={() => setAttempt(attempt + 1)}>{chatText.retry}</Button></div>
        : root === null ? chatText.loading : root.length ? root.map(e => <Node key={`${folder}/${e.name}`} path={folder} entry={e} depth={0} />) : chatText.treeEmpty}
    </div>
  </>;
}
