import os, re, subprocess, sys

ROOT = r"C:\GitHub\AI_Hub"
UI = os.path.join(ROOT, "ui")

def p(*a): return os.path.join(UI, *a)

def git_mv(src, dst):
    os.makedirs(os.path.dirname(p(dst)), exist_ok=True)
    subprocess.run(["git", "mv", "ui/" + src, "ui/" + dst], cwd=ROOT, check=True)

JS = {
    "js/api.js": "shared/api.js",
    "js/state.js": "shared/state.js",
    "js/util.js": "shared/util.js",
    "js/main.js": "main.js",
    "js/chat.js": "page/home/chat/chat.js",
    "js/mode-chip.js": "components/composer/mode-chip.js",
    "js/debate-card.js": "components/debate-card/debate-card.js",
    "js/debate-actions.js": "components/debate-card/debate-actions.js",
    "js/plan-card.js": "components/plan-card/plan-card.js",
    "js/plan-helpers.js": "components/plan-card/plan-helpers.js",
    "js/plan-order.js": "components/plan-card/plan-order.js",
    "js/plan-summary.js": "components/plan-card/plan-summary.js",
    "js/plans.js": "page/plans/plans.js",
    "js/debate.js": "page/debate/debate.js",
    "js/dev.js": "page/dev/dev.js",
    "js/mutis-page.js": "page/mutis/mutis.js",
    "js/settings.js": "page/settings/settings.js",
    "js/extensions.js": "page/settings/extensions/extensions.js",
    "js/memory.js": "page/settings/memory/memory.js",
    "components/sidebar/sidebar.js": "components/sidebar/sidebar.js",
}
CSS_WHOLE = {
    "styles/base.css": "shared/base.css",
    "styles/layout.css": "shared/layout.css",
    "styles/plan-card.css": "components/plan-card/plan-card.css",
    "styles/debate-card.css": "components/debate-card/debate-card.css",
    "styles/memory.css": "page/settings/memory/memory.css",
    "styles/dev.css": "page/dev/dev.css",
    "styles/settings.css": "page/settings/settings.css",
    "styles/extensions.css": "page/settings/extensions/extensions.css",
    "styles/plans.css": "page/plans/plans.css",
    "styles/mutis.css": "page/mutis/mutis.css",
    "styles/chat.css": "page/home/chat/chat.css",
}
for s, d in {**JS, **CSS_WHOLE}.items():
    if s != d:
        git_mv(s, d)

base_to_new = {os.path.basename(s): "/" + d for s, d in JS.items()}
imp = re.compile(r'(from\s+)(["\'])([^"\']+\.js)\2')
def fix(m):
    name = os.path.basename(m.group(3))
    if name not in base_to_new:
        raise SystemExit("unknown import " + m.group(3))
    return f'{m.group(1)}"{base_to_new[name]}"'
for new in JS.values():
    f = p(new)
    t = open(f, encoding="utf-8").read()
    open(f, "w", encoding="utf-8", newline="").write(imp.sub(fix, t))

def blocks(text):
    out, depth, cur = [], 0, ""
    for ch in text:
        cur += ch
        if ch == "{": depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                out.append(cur.strip("\n")); cur = ""
        elif ch == ";" and depth == 0 and cur.strip().startswith("@"):
            out.append(cur.strip("\n")); cur = ""
    if cur.strip(): out.append(cur.strip("\n"))
    return out

def sel(b): return b.split("{")[0].strip()

HERO = re.compile(r'^\.welcome')
TOOLBAR = re.compile(r'^\.(toolbar|control-group|model-control|folder-control|permission-control)\b')
COMPOSER = re.compile(r'^(\.composer|\.running-|\.attachment-|\.mode-|\.effort-|#mode-|\.plan-hint|#debate-button|\.mutis-role-row|\.mutis-chip|\.stop-button\.stop-emphasis|@keyframes running-pulse)')
HISTORY = re.compile(r'^\.(section-label|chat-list|chat-item|chat-select|chat-delete)\b')
SETTINGS_BTN = re.compile(r'^\.settings-open(\s|$)')
EXTVIEW = re.compile(r'^\.extensions-(view|header)')

buckets = {k: [] for k in ["hero", "toolbar", "composer", "history", "settingsbtn", "extview"]}
def route(rel, history=False):
    text = open(p(rel), encoding="utf-8").read()
    kept = []
    for b in blocks(text):
        s = sel(b)
        if s.startswith("@media"):
            kept.append(b); continue
        if HERO.match(s): buckets["hero"].append(b)
        elif TOOLBAR.match(s): buckets["toolbar"].append(b)
        elif COMPOSER.match(s) or s.startswith(".mode-popover:has"): buckets["composer"].append(b)
        elif history and HISTORY.match(s): buckets["history"].append(b)
        elif history and SETTINGS_BTN.match(s): buckets["settingsbtn"].append(b)
        elif EXTVIEW.match(s): buckets["extview"].append(b)
        else: kept.append(b)
    open(p(rel), "w", encoding="utf-8", newline="").write("\n".join(kept) + "\n")

route("shared/layout.css")
route("page/home/chat/chat.css")
route("page/plans/plans.css")
route("page/mutis/mutis.css")
route("page/settings/extensions/extensions.css")
route("components/sidebar/sidebar.css", history=True)

def write_new(rel, header, items):
    os.makedirs(os.path.dirname(p(rel)), exist_ok=True)
    open(p(rel), "w", encoding="utf-8", newline="").write(f"/* {header} */\n" + "\n".join(items) + "\n")

write_new("components/hero/hero.css", "Hero: màn chào khi chưa có tin nhắn", buckets["hero"])
write_new("components/toolbar/toolbar.css", "Thanh công cụ: nguồn, model, thư mục, quyền", buckets["toolbar"])
write_new("components/composer/composer.css", "Khung soạn tin, chọn chế độ (mode-chip), tệp đính kèm", buckets["composer"])
write_new("components/sidebar/history/history.css", "Sidebar: khung lịch sử chat", buckets["history"])
write_new("components/sidebar/settings/settings-button.css", "Sidebar: nút Cài đặt", buckets["settingsbtn"])
# extensions-view/header dùng chung cho các trang toàn màn hình (MutiS, Tự phát triển)
with open(p("shared/layout.css"), "a", encoding="utf-8", newline="") as f:
    f.write("/* Trang toàn màn hình (MutiS, Tự phát triển) */\n" + "\n".join(buckets["extview"]) + "\n")
print({k: len(v) for k, v in buckets.items()})
