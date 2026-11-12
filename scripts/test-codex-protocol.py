"""Read-only protocol smoke test against a locally installed Codex executable.
Uses a disposable app home under target; never reads or copies personal credentials.
"""
import json
import os
import queue
import subprocess
import sys
import threading
from pathlib import Path

exe = sys.argv[1]
root = Path("target/codex-protocol-smoke").resolve()
home = root / "home"
profile = home / "profile"
project = root / "project"
for directory in (home, profile, project):
    directory.mkdir(parents=True, exist_ok=True)
(project / ".git").mkdir(exist_ok=True)
(project / "AGENTS.md").write_text("PERSONAL_MARKER_MUST_NOT_BE_LOADED", encoding="utf-8")
configs = [
    'cli_auth_credentials_store="file"', 'project_doc_max_bytes=0',
    'skills.include_instructions=false', 'skills.bundled.enabled=false',
    'features.apps=false', 'features.plugins=false', 'memories.use_memories=false',
    'mcp_servers={}', 'projects.' + json.dumps(str(project)) + '.trust_level="untrusted"',
]
args = [exe, "app-server", "--listen", "stdio://"]
for config in configs:
    args += ["-c", config]
keep = ["PATH", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "TEMP", "TMP", "APPDATA", "LOCALAPPDATA"]
env = {key: os.environ[key] for key in keep if key in os.environ}
env.update(CODEX_HOME=str(home), HOME=str(profile), USERPROFILE=str(profile))
process = subprocess.Popen(args, cwd=home, env=env, stdin=subprocess.PIPE,
    stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding="utf-8",
    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
messages = queue.Queue()
def reader():
    for line in process.stdout:
        try:
            messages.put(json.loads(line))
        except ValueError:
            pass
threading.Thread(target=reader, daemon=True).start()
serial = 0
def call(method, params):
    global serial
    serial += 1
    current = serial
    process.stdin.write(json.dumps(dict(id=current, method=method, params=params)) + "\n")
    process.stdin.flush()
    while True:
        response = messages.get(timeout=30)
        if response.get("id") == current and "method" not in response:
            assert "error" not in response, (method, response.get("error", {}).get("code"))
            return response["result"]
try:
    call("initialize", {"clientInfo":{"name":"mutis_protocol_test","version":"0.1.0"},"capabilities":{"experimentalApi":True}})
    process.stdin.write('{"method":"initialized"}\n')
    process.stdin.flush()
    auth = call("account/read", {"refreshToken":False})
    assert auth.get("account") is None, "Disposable home must not inherit a personal account"
    skills = call("skills/list", {"cwds":[str(project)],"forceReload":True})
    for entry in skills.get("data", []):
        assert not entry.get("skills"), "Disposable home unexpectedly exposed skills"
    # Thread creation tests transport and sandbox configuration without starting inference.
    thread = call("thread/start", {"cwd":str(project),"model":"gpt-6.1-sol", "approvalPolicy":"never", "sandbox":"read-only", "selectedCapabilityRoots":[]})
    assert thread["thread"]["id"]
    print("PASS Codex app-server: initialize, isolated signed-out account, empty skills, read-only thread creation; no inference requested")
finally:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
