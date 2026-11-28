type TauriWindow = {
  setIcon(icon: Uint8Array): Promise<void>;
  requestUserAttention(kind: 2 | null): Promise<void>;
};

const tauriWindow = () => (window as Window & {
  __TAURI__?: { window?: { getCurrentWindow(): TauriWindow } };
}).__TAURI__?.window;

let count = 0;
let asking = false;
let listening = false;
let badgeVersion = 0;
let originalBytes: Uint8Array | undefined;
let iconImage: Promise<ImageBitmap> | undefined;

async function loadIcon(): Promise<ImageBitmap> {
  const response = await fetch("/brand/app-icon-128.png");
  if (!response.ok) throw new Error("App icon unavailable");
  const blob = await response.blob();
  originalBytes = new Uint8Array(await blob.arrayBuffer());
  return createImageBitmap(blob);
}

async function updateAttention(kind: 2 | null) {
  try { await tauriWindow()?.getCurrentWindow().requestUserAttention(kind); }
  catch { /* Attention may be unavailable. */ }
}

async function updateIcon(icon: Uint8Array) {
  try { await tauriWindow()?.getCurrentWindow().setIcon(icon); }
  catch { /* The window icon may be unavailable. */ }
}

async function drawBadge(version: number) {
  try {
    const image = await (iconImage ??= loadIcon());
    if (version !== badgeVersion || document.hasFocus()) return;
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 128;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.drawImage(image, 0, 0, 128, 128);
    ctx.fillStyle = asking ? "#f59e0b" : "#e5484d";
    ctx.beginPath();
    ctx.arc(98, 30, 28, 0, 2 * Math.PI);
    ctx.fill();
    ctx.lineWidth = 5;
    ctx.strokeStyle = "#202020";
    ctx.stroke();
    ctx.fillStyle = "#fff";
    ctx.font = count > 9 ? "bold 32px sans-serif" : "bold 40px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(count > 9 ? "9+" : String(count), 98, 30);

    const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, "image/png"));
    if (!blob) return;
    const bytes = new Uint8Array(await blob.arrayBuffer());
    if (version === badgeVersion && !document.hasFocus()) await updateIcon(bytes);
  } catch { /* Badge rendering is best effort. */ }
}

export function desktopAttention(kind: "done" | "question"): void {
  if (typeof window === "undefined" || !tauriWindow() || document.hasFocus()) return;
  if (!listening) {
    listening = true;
    window.addEventListener("focus", () => {
      count = 0;
      asking = false;
      badgeVersion++;
      if (originalBytes) void updateIcon(originalBytes);
      void updateAttention(null);
    });
  }

  count++;
  if (kind === "question") asking = true;
  void drawBadge(++badgeVersion);
  void updateAttention(2);
}
