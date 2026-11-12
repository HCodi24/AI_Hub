/** Pets that ship inside the app (own artwork, `web/public/pets/<id>/spritesheet.webp, lossless`): nothing is downloaded or imported. */
export const BUNDLED_PETS: Record<string, { name: string; note?: string; icon?: string }> = {
  "pixel-cloud": { name: "Pixel Cloud" },
  /** A single image instead of a sprite sheet: the avatar just moves it with CSS. */
  "deepseek-whale": { name: "Cá voi DeepSeek", icon: "/app/pets/deepseek-whale/icon.webp" },
  "clawd-svg": { name: "Clawd", note: "Nhân vật Clawd thuộc bản quyền Anthropic (bản fan không chính thức): chỉ dùng cá nhân. Thư mục này bị git bỏ qua; đừng đưa lên repo hay phát hành bản build chứa nó." },
};
/** Where a pet's sprite sheet is loaded from: the app's own files, else the backend (downloaded / installed pets). */
export const petSheetUrl = (id: string) => id in BUNDLED_PETS ? `/app/pets/${id}/spritesheet.webp` : `/api/pets/${id}`;
/** Pet used when the user has not chosen one, per source kind. */
export const DEFAULT_PETS: Record<string, string> = { codex_cli: "pixel-cloud", claude_cli: "clawd-svg", anthropic: "clawd-svg", deepseek: "deepseek-whale" };
export const PET_KINDS = ["codex_cli", "claude_cli", "anthropic", "deepseek", "openai_compat"] as const;
/** The agent loops report their own kinds; they belong to the OpenAI-compatible source for the pet choice. */
export const petKind = (provider: string) => provider === "deepseek_agent" || provider === "mcp_agent" ? "openai_compat" : provider;
/** Clawd's idle animation is 6x faster than the Codex-format default. */
export const idleMs = (pet: string) => pet === "clawd-svg" ? 1100 : 6600;
