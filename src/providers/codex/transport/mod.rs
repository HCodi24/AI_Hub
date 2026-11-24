//! JSON-RPC transport. Raw auth/protocol errors never reach UI logs.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin},
    sync::mpsc,
};

/// A `thread/resume` answer carries the whole thread, so a long worker thread alone passed the old 4 MB cap (4.2 MB) and every resume failed.
const MAX_LINE: usize = 64 * 1024 * 1024;

pub(crate) struct Client {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Value>,
    queued: VecDeque<Value>,
    serial: u64,
    log: std::path::PathBuf,
}

/// Diagnostics only: why the app-server connection ended (the app-server's stderr is otherwise thrown away).
fn note(log: &Path, text: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(file, "[{}] {text}", crate::core::time::now_ms());
    }
}

impl Client {
    pub async fn start(executable: &Path, home: &Path, folder: &Path) -> Result<Self> {
        let profile = home.join("profile");
        std::fs::create_dir_all(&profile)?;
        let args = super::environment::launch_args(folder, super::environment::has_plugins(home));
        let mut command = super::super::cli_support::command_for(executable, &args);
        configure_environment(&mut command, home, &profile);
        command
            .current_dir(home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .context("Không khởi động được Codex app-server")?;
        let input = child.stdin.take().context("Không mở được kết nối Codex")?;
        let stdout = child
            .stdout
            .take()
            .context("Không đọc được kết nối Codex")?;
        let log = home.join("transport.log");
        note(&log, &format!("start pid={:?} folder={}", child.id(), folder.display()));
        if let Some(stderr) = child.stderr.take() {
            let log = log.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    note(&log, &format!("stderr: {}", crate::core::text::truncate_chars(&line, 500)));
                }
            });
        }
        let (tx, output) = mpsc::channel(256);
        let reader_log = log.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let reason = loop {
                match lines.next_line().await {
                    Ok(Some(line)) if line.len() > MAX_LINE => break format!("line over {MAX_LINE} bytes ({} bytes)", line.len()),
                    Ok(Some(line)) => match serde_json::from_str(&line) {
                        Ok(value) => {
                            if tx.send(value).await.is_err() {
                                break "receiver dropped".to_owned();
                            }
                        }
                        Err(error) => note(&reader_log, &format!("unparsed stdout line ({} bytes): {error}", line.len())),
                    },
                    Ok(None) => break "stdout closed (process exited)".to_owned(),
                    Err(error) => break format!("stdout read error: {error}"),
                }
            };
            note(&reader_log, &format!("reader ended: {reason}"));
        });
        let mut client = Self {
            child,
            input,
            output,
            queued: VecDeque::new(),
            serial: 0,
            log,
        };
        if let Err(error) = client.call("initialize", json!({"clientInfo":{"name":"mutis","version":"0.1.0"},"capabilities":{"experimentalApi":true}})).await {
            client.close().await;
            return Err(error);
        }
        client.write(json!({"method":"initialized"})).await?;
        Ok(client)
    }

    pub async fn write(&mut self, value: Value) -> Result<()> {
        self.input
            .write_all(format!("{value}\n").as_bytes())
            .await?;
        self.input.flush().await?;
        Ok(())
    }

    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.serial += 1;
        let id = self.serial;
        self.write(json!({"id":id,"method":method,"params":params}))
            .await?;
        let result = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let Some(value) = self.output.recv().await else {
                    self.note_exit(method);
                    bail!("Codex đã đóng kết nối");
                };
                if value.get("id").and_then(Value::as_u64) == Some(id) && value.get("method").is_none() {
                    if value.get("error").is_some() { bail!("Codex không thực hiện được {method}; kiểm tra phiên bản và trạng thái đăng nhập"); }
                    return Ok(value["result"].clone());
                }
                if value.get("id").is_some() && value.get("method").is_some() {
                    self.reject_request(&value).await?;
                } else {
                    if self.queued.len() >= 1024 { bail!("Quá nhiều sự kiện Codex chưa xử lý"); }
                    self.queued.push_back(value);
                }
            }
        }).await.context("Codex không phản hồi trong 30 giây")?;
        result
    }

    pub async fn next(&mut self) -> Result<Value> {
        if let Some(value) = self.queued.pop_front() {
            return Ok(value);
        }
        match self.output.recv().await {
            Some(value) => Ok(value),
            None => {
                self.note_exit("next");
                bail!("Codex kết thúc trước khi hoàn thành lượt trả lời")
            }
        }
    }

    fn note_exit(&mut self, during: &str) {
        let status = self.child.try_wait();
        note(&self.log, &format!("connection closed during {during}; process status: {status:?}"));
    }

    pub fn login_failed(&mut self) -> bool {
        let failed = self
            .queued
            .iter()
            .any(|v| v["method"] == "account/login/completed" && v["params"]["success"] == false);
        self.queued.clear();
        failed
    }

    pub async fn respond(&mut self, request: &Value, result: Value) -> Result<()> {
        self.write(json!({"id":request["id"],"result":result})).await
    }

    pub async fn reject_request(&mut self, value: &Value) -> Result<()> {
        // Permissions remain controlled by the app. Unsupported native prompts must not hang.
        let response = if value["method"]
            .as_str()
            .unwrap_or("")
            .contains("requestApproval")
        {
            json!({"id":value["id"],"result":{"decision":"decline"}})
        } else {
            json!({"id":value["id"],"error":{"code":-32601,"message":"Use the MutiS question card for user decisions; this host tool is unavailable."}})
        };
        self.write(response).await
    }

    pub async fn close(&mut self) {
        super::super::cli_support::terminate_child_tree(&mut self.child).await;
    }
}

fn configure_environment(command: &mut tokio::process::Command, home: &Path, profile: &Path) {
    let injected = command
        .as_std()
        .get_envs()
        .filter_map(|(key, value)| value.map(|v| (key.to_owned(), v.to_owned())))
        .collect::<Vec<_>>();
    let core = [
        "PATH",
        "SystemRoot",
        // Without these a Windows tool run by Codex writes a literal "%SystemDrive%\ProgramData\..." folder into the project.
        "SystemDrive",
        "ProgramData",
        "ALLUSERSPROFILE",
        "WINDIR",
        "COMSPEC",
        "PATHEXT",
        "TEMP",
        "TMP",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "SSL_CERT_FILE",
        "CODEX_CA_CERTIFICATE",
    ]
    .into_iter()
    .filter_map(|key| std::env::var_os(key).map(|value| (key, value)))
    .collect::<Vec<_>>();
    command.env_clear().envs(core).envs(injected);
    command
        .env("CODEX_HOME", home)
        .env("HOME", profile)
        .env("USERPROFILE", profile)
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CODEX_INTERNAL_ORIGINATOR_OVERRIDE")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL");
}

#[cfg(test)]
mod tests;
