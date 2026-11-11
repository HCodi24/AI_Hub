"""Render the actual static React export against isolated local fixtures. No AI/network services."""
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from pathlib import Path
import json
import os
import subprocess
import threading
import urllib.parse

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target/codex/screenshots-r2"
OUT.mkdir(parents=True, exist_ok=True)
CHAT = "00000000-0000-4000-8000-000000000001"
PLAN = "00000000-0000-4000-8000-000000000002"
DEBATE = "00000000-0000-4000-8000-000000000003"
providers = [{"id": "codex", "name": "Codex CLI", "kind": "codex_cli", "models": ["mock-codex"]}, {"id": "claude", "name": "Claude CLI", "kind": "claude_cli", "models": ["mock-claude"]}]
role = {"provider_id": "claude", "model": "mock-claude", "effort": "high"}
mutis = {"enabled": True, "roles": {"lead": role, "big": {**role, "provider_id": "codex"}, "small": role}, "auto_plan": True, "verify_gate": True, "auto_review": True, "auto_fix": True}
plan = {"id": PLAN, "chat_id": CHAT, "title": "Bổ sung bộ nhớ dự án", "folder": "C:/du-an/AI_Hub", "request": "Thêm bộ nhớ dự án, giữ tiếng Việt và kiểm tra giao diện di động.", "approved": False, "tasks": [
    {"id": 1, "title": "Khảo sát luồng chat và bộ nhớ", "detail": "Đọc cấu trúc hiện tại và xác nhận các API lưu trữ.", "assignee": "claude", "kind": "plan", "status": "xong", "summary": "Đã xác nhận các API đang dùng. Giữ nguyên giao diện cũ.", "tokens": 12345},
    {"id": 2, "title": "Triển khai giao diện React", "detail": "Thêm thẻ kế hoạch, bộ nhớ và trạng thái đang chạy.", "assignee": "codex", "kind": "code", "depends_on": [1], "status": "chờ", "permission": "full"},
    {"id": 3, "title": "Kiểm tra tiếng Việt và màn hình nhỏ", "detail": "Xác minh 375px, theme sáng và tối.", "assignee": "claude", "kind": "test", "depends_on": [2], "status": "chờ"}]}
turn = {"text": "### Phương án đề xuất\nGiữ **API Rust hiện tại** và chia giao diện theo tính năng.\n\n- Dùng React Query cho lịch sử.\n- Dùng Zustand cho thẻ đang chạy.\n- Kiểm tra tiếng Việt và màn hình 375px.\n\n```ts\nconst permission = \"full\";\n```", "agree": True, "tokens": 523456, "elapsed_s": 42}
debate = {"id": DEBATE, "chat_id": CHAT, "title": "Thiết kế bộ nhớ dự án", "request": plan["request"], "folder": plan["folder"], "status": "done", "elapsed_s": 98, "codex_model": "mock-codex", "claude_model": "mock-claude", "judge_model": "mock-judge", "max_rounds": 2, "rounds": [{"index": 1, "codex": turn, "claude": {**turn, "tokens": 34876, "text": "### Đánh giá phương án\nĐồng ý giữ API hiện tại. Cần thêm **Hoàn tác** khi lưu trữ scope và xác nhận trước khi xoá.\n\nKiểm tra Escape, focus và giao diện di động."}}, {"index": 2, "codex": {**turn, "tokens": 18420}, "claude": {**turn, "tokens": 15280}}], "verdict": {"decision": "Kết hợp hai phương án: giữ API hiện có, dùng thẻ React trong chat và cung cấp bộ nhớ theo thư mục dự án.", "chosen": "merged", "agreed_points": ["Giữ legacy UI hoạt động", "Không gọi AI trong kiểm tra tự động"], "disagreements": ["Thứ tự trình bày phần biên bản"], "risks": ["Tránh mất trạng thái khi đang stream"], "plan_steps": ["Định nghĩa schema và API", "Triển khai thẻ trong chat", "Kiểm tra offline và chụp giao diện"]}, "plan_request": plan["request"]}
memory = {"project": "r2-fixture", "index": "# Bộ nhớ dự án", "scopes": [{"scope": "react-r2", "title": "Giao diện React trong chat", "status": "active", "updated": "2026-11-11T00:00:00Z", "next": "Kiểm tra màn hình nhỏ"}], "archived": [{"scope": "react-r1", "title": "Nền tảng chat React", "status": "done", "updated": "2026-11-10T00:00:00Z", "next": ""}]}
scope_text = "---\nscope: react-r2\ntitle: Giao diện React trong chat\nstatus: active\n---\n## Mục tiêu\n- Đưa tranh luận và kế hoạch vào chat.\n## Đã chốt\n- Dùng API Rust hiện tại.\n## Đã làm\n- Hoàn tất schema và giao diện bộ nhớ.\n## Còn mở\n- Kiểm tra responsive 375px."
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass
    def do_POST(self):
        if self.path.endswith("/test"):
            self.send_json({"status": "connected"})
        else:
            self.send_error(405, "Fixture refuses mutations and AI calls")
    def send_json(self, value):
        self.send_bytes(json.dumps(value, ensure_ascii=False).encode(), "application/json")
    def send_bytes(self, content, mime):
        self.send_response(200)
        self.send_header("Content-Type", mime)
        self.end_headers()
        self.wfile.write(content)
    def do_GET(self):
        path = urllib.parse.urlsplit(self.path).path
        mode, theme = self.server.fixture
        if path == "/api/config": return self.send_json({"providers": providers, "default_folder": plan["folder"]})
        if path == "/api/mutis": return self.send_json(mutis)
        if path == "/api/chats": return self.send_json([{"id": CHAT, "title": "R2 fixture", "updated_at": 1}])
        if path == "/api/chats/" + CHAT:
            msg = {"role": "assistant", "content": "", "debate_id" if mode == "debate" else "plan_id": DEBATE if mode == "debate" else PLAN}
            return self.send_json({"id": CHAT, "title": "R2 fixture", "updated_at": 1, "provider_id": "codex", "model": "mock-codex", "folder": plan["folder"], "messages": [] if mode == "memory" else [msg]})
        if path == "/api/debates/" + DEBATE: return self.send_json(debate)
        if path == "/api/plans/" + PLAN: return self.send_json(plan)
        if path == "/api/memory": return self.send_json(memory)
        if path.startswith("/api/memory/"): return self.send_json({"text": scope_text})
        rel = path.removeprefix("/app/") or "index.html"
        asset = (ROOT / "web/out" / rel).resolve()
        if asset.is_dir(): asset = asset / "index.html"
        if not asset.is_relative_to((ROOT / "web/out").resolve()) or not asset.is_file(): return self.send_error(404)
        import mimetypes
        mime = mimetypes.guess_type(asset.name)[0] or "application/octet-stream"
        data = asset.read_bytes()
        if asset.suffix == ".html":
            setup = f"localStorage.setItem('ai-hub-theme','{theme}'); document.documentElement.dataset.theme='{theme}';"
            automate = """
let loaded = false, opened = false, edited = false;
setInterval(() => {
  const buttons = [...document.querySelectorAll('button')];
  if (!loaded) { const b = buttons.find(b => b.textContent.includes('R2 fixture')); if (b) { b.click(); loaded = true; } }
  MODE_MEMORY
  const report = document.querySelector('#layout-report') || document.body.appendChild(Object.assign(document.createElement('output'), {id:'layout-report'}));
  report.hidden = true; report.dataset.overflow = String(document.documentElement.scrollWidth > innerWidth);
}, 100);
"""
            memory_steps = """
if (loaded && !opened) { const b = buttons.find(b => b.textContent === 'Bộ nhớ'); if (b && !b.disabled) { b.click(); opened = true; } }
if (opened && !edited) { const b = buttons.find(b => b.textContent.includes('[active] react-r2')); if (b) { b.click(); edited = true; } }
"""
            automate = automate.replace("MODE_MEMORY", memory_steps if mode == "memory" else "")
            data = data.replace(b"<head>", ("<head><script>" + setup + "</script>").encode()).replace(b"</body>", ("<script>" + automate + "</script></body>").encode())
        self.send_bytes(data, mime)

def main():
    chrome = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Google/Chrome/Application/chrome.exe"
    chrome = Path(os.environ.get("AI_HUB_SCREENSHOT_BROWSER", str(chrome)))
    if not chrome.exists(): raise SystemExit("Chrome missing; no package installation attempted")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        for mode in ("debate", "plan", "memory"):
            for width, theme in ((1100, "dark"), (375, "light")):
                server.fixture = (mode, theme)
                name = f"{mode}-{width}-{theme}"
                args = [str(chrome), "--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--disable-background-networking", "--disable-component-update", "--hide-scrollbars", f"--user-data-dir={OUT / ('profile-' + chrome.stem + '-' + name)}", f"--window-size={width},1250", "--force-device-scale-factor=1", "--virtual-time-budget=10000", f"--screenshot={OUT / (name + '.png')}", "--dump-dom", f"http://127.0.0.1:{server.server_port}/app/"]
                process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    stdout, stderr = process.communicate(timeout=45)
                except subprocess.TimeoutExpired:
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True)
                    raise
                (OUT / (name + ".html")).write_bytes(stdout)
                if process.returncode: raise RuntimeError(f"{chrome.name} exited {process.returncode}: " + stderr.decode(errors="replace")[-1000:])
                overflow = b'data-overflow="true"' in stdout
                if overflow: raise RuntimeError(f"Horizontal overflow: {name}")
                if b'data-overflow="false"' not in stdout: raise RuntimeError(f"Fixture did not initialize: {name}")
                print(f"PASS {name}: screenshot and no horizontal overflow", flush=True)
    finally:
        server.shutdown()
        server.server_close()
if __name__ == "__main__": main()
