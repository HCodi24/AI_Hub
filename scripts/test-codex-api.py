"""HTTP smoke test with an isolated config and no personal credentials or inference."""
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
import uuid
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
codex = str(Path(sys.argv[2]).resolve())
root = Path("target/codex-http-smoke").resolve() / str(uuid.uuid4())
root.mkdir(parents=True)
config_path = root / "config.json"
config_path.write_text(json.dumps({"providers":[{"id":"codex-test","name":"Test Codex","kind":"codex_cli","executable":codex}]}), encoding="utf-8")
env = dict(os.environ)
env.update(AI_HUB_CONFIG_PATH=str(config_path), AI_HUB_DEV="0", CODEX_HOME=str(root / "empty-personal-home"))
process = subprocess.Popen([str(binary), "--port", "0", "--no-open"], env=env,
    stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding="utf-8",
    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
try:
    startup = process.stdout.readline()
    match = re.search(r"http://127\.0\.0\.1:\d+", startup)
    assert match, "Backend did not announce its test port"
    base = match.group()
    def request(method, path="/api/providers/codex-test/codex", data=None):
        body = json.dumps(data).encode() if data is not None else None
        req = urllib.request.Request(base + path, data=body, method=method, headers={"Content-Type":"application/json"})
        with urllib.request.urlopen(req, timeout=45) as response:
            return json.load(response)

    status = request("GET")
    assert not status["connected"] and not status["pending"]
    assert not status["project_skills"] and not status["project_instructions"]
    status = request("PUT", data={"project_skills": True, "project_instructions": False})
    assert status["project_skills"] and not status["project_instructions"]
    stored = json.loads(config_path.read_text(encoding="utf-8"))
    assert stored["codex"]["project_skills"] and stored["codex"]["revision"] == 1
    test = request("POST", "/api/providers/codex-test/test")
    assert not test["ok"], "An installed binary alone must not count as signed in"
    try:
        request("POST", data={"action":"cli"})
        raise AssertionError("Empty personal home unexpectedly imported credentials")
    except urllib.error.HTTPError as error:
        assert error.code == 502
        message = json.load(error)["error"]
        assert "web" in message
    login = request("POST", data={"action":"web"})
    assert login["pending"] and not login["connected"]
    assert login["url"].startswith(("https://auth.openai.com/", "https://chatgpt.com/", "https://auth0.openai.com/"))
    cancelled = request("POST", data={"action":"cancel"})
    assert not cancelled["pending"] and not cancelled["connected"]
    status = request("PUT", data={"project_skills": False, "project_instructions": True})
    assert not status["project_skills"] and status["project_instructions"]
    assert "tokens" not in json.dumps(status)
    print("PASS Codex HTTP: signed-out detection, persisted independent switches, CLI auth fallback, browser login URL and cancellation; no personal credentials or inference")
finally:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
