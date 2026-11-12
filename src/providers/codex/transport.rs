//! JSON-RPC transport. Raw auth/protocol errors never reach UI logs.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin},
    sync::mpsc,
};

pub(crate) struct Client {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Value>,
    queued: VecDeque<Value>,
    serial: u64,
}

impl Client {
    pub async fn start(executable: &Path, home: &Path, folder: &Path) -> Result<Self> {
        let profile = home.join("profile");
        std::fs::create_dir_all(&profile)?;
        let args = super::environment::launch_args(folder);
        let mut command = super::super::cli_support::command_for(executable, &args);
        let injected = command
            .as_std()
            .get_envs()
            .filter_map(|(key, value)| value.map(|v| (key.to_owned(), v.to_owned())))
            .collect::<Vec<_>>();
        let core = [
            "PATH",
            "SystemRoot",
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
            .env("HOME", &profile)
            .env("USERPROFILE", &profile)
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CODEX_INTERNAL_ORIGINATOR_OVERRIDE")
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .current_dir(home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .context("Không khởi động được Codex app-server")?;
        let input = child.stdin.take().context("Không mở được kết nối Codex")?;
        let stdout = child
            .stdout
            .take()
            .context("Không đọc được kết nối Codex")?;
        let (tx, output) = mpsc::channel(256);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.len() > 4 * 1024 * 1024 {
                    break;
                }
                if let Ok(value) = serde_json::from_str(&line) {
                    if tx.send(value).await.is_err() {
                        break;
                    }
                }
            }
        });
        let mut client = Self {
            child,
            input,
            output,
            queued: VecDeque::new(),
            serial: 0,
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
                let value = self.output.recv().await.context("Codex đã đóng kết nối")?;
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
        self.output
            .recv()
            .await
            .context("Codex kết thúc trước khi hoàn thành lượt trả lời")
    }

    pub fn login_failed(&mut self) -> bool {
        let failed = self
            .queued
            .iter()
            .any(|v| v["method"] == "account/login/completed" && v["params"]["success"] == false);
        self.queued.clear();
        failed
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Requires MUTIS_CODEX_TEST_EXE; uses a disposable home and no inference"]
    async fn installed_app_server_uses_an_isolated_home() {
        let executable =
            std::env::var_os("MUTIS_CODEX_TEST_EXE").expect("Set MUTIS_CODEX_TEST_EXE");
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/codex-live-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let mut client = Client::start(Path::new(&executable), &root, &root)
            .await
            .unwrap();
        let account = client
            .call("account/read", json!({"refreshToken":false}))
            .await
            .unwrap();
        assert!(
            account["account"].is_null(),
            "New MutiS home must not inherit personal authentication"
        );
        let thread = client.call("thread/start",json!({"cwd":root,"model":"gpt-6.1-sol","sandbox":"read-only","approvalPolicy":"never","selectedCapabilityRoots":[]})).await.unwrap();
        assert!(thread["thread"]["id"].as_str().is_some());
        let login = client
            .call("account/login/start", json!({"type":"chatgpt"}))
            .await
            .unwrap();
        let url = reqwest::Url::parse(login["authUrl"].as_str().unwrap()).unwrap();
        assert_eq!(url.scheme(), "https");
        assert!(matches!(
            url.host_str(),
            Some("auth.openai.com" | "chatgpt.com" | "auth0.openai.com")
        ));
        assert!(login["loginId"].as_str().is_some());
        client
            .call("account/login/cancel", json!({"loginId":login["loginId"]}))
            .await
            .unwrap();
        client.close().await;
    }
}
