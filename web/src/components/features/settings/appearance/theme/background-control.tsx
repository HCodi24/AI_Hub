import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { ImagePlus, RotateCcw } from "lucide-react";
import { notify } from "@/components/toast/toast";
import { settingsText as t } from "@/text/settings";
import styles from "./background-control.module.css";

const BG_STORAGE_KEY = "ai-hub-glass-bg";
const DEFAULT_BG_URL = "/app/backgrounds/default-glass.webp";
const MAX_RAW_BYTES = 1_500_000;

const readAsDataUrl = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = () => reject(reader.error ?? new Error("read failed"));
  reader.onload = () => resolve(String(reader.result ?? ""));
  reader.readAsDataURL(file);
});

function resizeDataUrl(dataUrl: string, maxEdge: number, quality: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onerror = () => reject(new Error("decode failed"));
    img.onload = () => {
      const scale = Math.min(1, maxEdge / Math.max(img.naturalWidth || 1, img.naturalHeight || 1));
      const width = Math.max(1, Math.round(img.naturalWidth * scale));
      const height = Math.max(1, Math.round(img.naturalHeight * scale));
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) { reject(new Error("canvas unavailable")); return; }
      ctx.drawImage(img, 0, 0, width, height);
      resolve(canvas.toDataURL("image/jpeg", quality));
    };
    img.src = dataUrl;
  });
}

function applyBackground(dataUrl: string | null) {
  const root = document.documentElement;
  if (dataUrl) root.style.setProperty("--glass-bg-image", `url(${JSON.stringify(dataUrl)})`);
  else root.style.removeProperty("--glass-bg-image");
}

function saveBackground(dataUrl: string | null) {
  if (dataUrl) localStorage.setItem(BG_STORAGE_KEY, dataUrl);
  else localStorage.removeItem(BG_STORAGE_KEY);
}

export function BackgroundControl() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [customBg, setCustomBg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    try {
      const saved = localStorage.getItem(BG_STORAGE_KEY);
      if (saved && saved.startsWith("data:image/")) {
        setCustomBg(saved);
        applyBackground(saved);
      }
    } catch { /* Storage unavailable. */ }
  }, []);

  async function onPick(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !file.type.startsWith("image/")) return;
    setBusy(true);
    try {
      let dataUrl = await readAsDataUrl(file);
      if (file.size > MAX_RAW_BYTES) {
        dataUrl = await resizeDataUrl(dataUrl, 1920, 0.86);
      }
      try {
        saveBackground(dataUrl);
      } catch {
        dataUrl = await resizeDataUrl(dataUrl, 1280, 0.78);
        saveBackground(dataUrl);
      }
      setCustomBg(dataUrl);
      applyBackground(dataUrl);
    } catch {
      notify(t.appearance.backgroundError);
    } finally {
      setBusy(false);
    }
  }

  function reset() {
    setCustomBg(null);
    applyBackground(null);
    try { saveBackground(null); } catch { /* Storage unavailable. */ }
  }

  return <div className={styles.control}>
    <input ref={inputRef} type="file" accept="image/*" hidden onChange={event => void onPick(event)} />
    <div className={styles.preview}>
      {/* eslint-disable-next-line @next/next/no-img-element -- local static/data-URL wallpaper preview */}
      <img src={customBg ?? DEFAULT_BG_URL} alt={t.appearance.background} />
    </div>
    <div className={styles.actions}>
      <button type="button" disabled={busy} onClick={() => inputRef.current?.click()}>
        <ImagePlus size={16} aria-hidden="true" /><span>{t.appearance.changeBackground}</span>
      </button>
      <button type="button" disabled={busy || !customBg} onClick={reset}>
        <RotateCcw size={16} aria-hidden="true" /><span>{t.appearance.resetBackground}</span>
      </button>
    </div>
  </div>;
}
