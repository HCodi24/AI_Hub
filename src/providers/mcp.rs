//! Minimal MCP client over stdio (newline-delimited JSON-RPC 2.0): initialize, tools/list, tools/call.
//! Lets in-app agents (API sources) use MCP servers without a Codex/Claude CLI in between.
use super::cli_support;
use crate::config::McpServerConfig;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
    time::timeout,
};

const START_TIMEOUT: Duration = Duration::from_secs(30);
const LIST_TIMEOUT: Duration = Duration::from_secs(20);
const CALL_TIMEOUT: Duration = Duration::from_secs(180);
/// A tool result goes back to the model in full, so cap it.
const MAX_RESULT_CHARS: usize = 20_000;
const PAGE_LIMIT: usize = 10;

pub struct McpTool {
    pub name: String,
    pub description: String,
    pub schema: Value,
    /// `annotations.readOnlyHint`: only such tools are offered in read-only mode.
    pub read_only: bool,
}

pub struct McpClient {
    pub name: String,
    // Held so the process is killed when the client is dropped.
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl McpClient {
    /// Starts the server in `folder` and performs the MCP handshake.
    pub async fn start(config: &McpServerConfig, folder: &str) -> Result<Self> {
        let path = cli_support::resolve_executable(&config.command, None)
            .unwrap_or_else(|| PathBuf::from(&config.command));
        let args: Vec<OsString> = config.args.iter().map(OsString::from).collect();
        let mut command = cli_support::command_for(&path, &args);
        command
            .current_dir(folder)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        crate::core::process::configure_hidden(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("Không chạy được {}", config.command))?;
        let stdin = child.stdin.take().context("Không mở được stdin của MCP")?;
        let stdout = child
            .stdout
            .take()
            .context("Không mở được stdout của MCP")?;
        let mut client = Self {
            name: config.name.clone(),
            _child: child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            next_id: 1,
        };
        timeout(START_TIMEOUT, async {
            client
                .request(
                    "initialize",
                    json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"mutis","version":"0.1"}}),
                )
                .await?;
            client.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await
        })
        .await
        .map_err(|_| anyhow!("MCP {} không phản hồi sau {} giây", config.name, START_TIMEOUT.as_secs()))??;
        Ok(client)
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        timeout(LIST_TIMEOUT, async {
            let mut tools = Vec::new();
            let mut cursor: Option<String> = None;
            for _ in 0..PAGE_LIMIT {
                let params = cursor.as_ref().map_or(json!({}), |c| json!({"cursor": c}));
                let result = self.request("tools/list", params).await?;
                tools.extend(
                    result["tools"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(parse_tool),
                );
                cursor = result["nextCursor"].as_str().map(str::to_owned);
                if cursor.is_none() {
                    break;
                }
            }
            Ok(tools)
        })
        .await
        .map_err(|_| anyhow!("MCP {} không trả danh sách công cụ kịp", self.name))?
    }

    /// Runs one tool and returns its text (an error result is returned as text starting with `error:`).
    pub async fn call(&mut self, tool: &str, arguments: Value) -> Result<String> {
        let result = timeout(
            CALL_TIMEOUT,
            self.request("tools/call", json!({"name": tool, "arguments": arguments})),
        )
        .await
        .map_err(|_| anyhow!("Công cụ MCP chạy quá {} giây", CALL_TIMEOUT.as_secs()))??;
        Ok(result_text(&result))
    }

    async fn send(&mut self, message: &Value) -> Result<()> {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .context("Không gửi được cho MCP")?;
        self.stdin.flush().await.context("Không gửi được cho MCP")
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        loop {
            let Some(line) = self.lines.next_line().await.context("Đọc MCP lỗi")? else {
                bail!("MCP {} đã đóng kết nối", self.name);
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            // The server may ask us things too: answer `ping`, refuse the rest, never block on it.
            if let (Some(method), Some(request_id)) =
                (message["method"].as_str(), message.get("id"))
            {
                let reply = if method == "ping" {
                    json!({"jsonrpc":"2.0","id":request_id,"result":{}})
                } else {
                    json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32601,"message":"Method not found"}})
                };
                self.send(&reply).await?;
                continue;
            }
            if message["id"] != json!(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                bail!(
                    "MCP {method}: {}",
                    error["message"].as_str().unwrap_or("lỗi không rõ")
                );
            }
            return Ok(message["result"].clone());
        }
    }
}

/// One tool of one running server, as offered to a model.
pub struct ToolRef {
    pub function: String,
    pub server: String,
    pub tool: String,
    description: String,
    schema: Value,
    client: usize,
}

/// The enabled MCP servers of one chat run, started together; they stop when this is dropped.
pub struct McpTools {
    clients: Vec<McpClient>,
    pub tools: Vec<ToolRef>,
}

impl McpTools {
    /// Starts every server in `folder` and lists its tools. A server that fails is logged and skipped.
    /// Without write permission only tools the server marks `readOnlyHint` are offered.
    pub async fn start(
        servers: &[McpServerConfig],
        folder: &str,
        can_write: bool,
        events: &tokio::sync::mpsc::Sender<super::ProviderEvent>,
    ) -> Self {
        let mut found = Self {
            clients: Vec::new(),
            tools: Vec::new(),
        };
        for server in servers {
            let started = async {
                let mut client = McpClient::start(server, folder).await?;
                let tools = client.list_tools().await?;
                anyhow::Ok((client, tools))
            }
            .await;
            match started {
                Ok((client, tools)) => {
                    let index = found.clients.len();
                    found.clients.push(client);
                    for tool in tools.into_iter().filter(|tool| can_write || tool.read_only) {
                        found.tools.push(ToolRef {
                            function: function_name(&server.name, &tool.name),
                            server: server.name.clone(),
                            tool: tool.name,
                            description: tool.description,
                            schema: tool.schema,
                            client: index,
                        });
                    }
                }
                Err(error) => {
                    let _ = super::emit_text(
                        events,
                        "log",
                        format!("MCP {} không dùng được: {error:#}", server.name),
                    )
                    .await;
                }
            }
        }
        found
    }

    /// OpenAI-style `tools` entries.
    pub fn openai_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| json!({"type":"function","function":{"name":t.function,"description":t.description,"parameters":t.schema}}))
            .collect()
    }

    /// Anthropic-style `tools` entries.
    pub fn anthropic_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| json!({"name":t.function,"description":t.description,"input_schema":t.schema}))
            .collect()
    }

    pub fn find(&self, function: &str) -> Option<&ToolRef> {
        self.tools.iter().find(|tool| tool.function == function)
    }

    pub async fn call(&mut self, function: &str, arguments: Value) -> Result<String> {
        let (client, tool) = self
            .find(function)
            .map(|t| (t.client, t.tool.clone()))
            .with_context(|| format!("Công cụ MCP không tồn tại: {function}"))?;
        self.clients[client].call(&tool, arguments).await
    }
}

fn parse_tool(value: &Value) -> Option<McpTool> {
    Some(McpTool {
        name: value["name"].as_str()?.to_owned(),
        description: value["description"].as_str().unwrap_or_default().to_owned(),
        schema: if value["inputSchema"].is_object() {
            value["inputSchema"].clone()
        } else {
            json!({"type":"object","properties":{}})
        },
        read_only: value["annotations"]["readOnlyHint"]
            .as_bool()
            .unwrap_or(false),
    })
}

/// Text of a `tools/call` result; an `isError` result is prefixed with `error:`.
fn result_text(result: &Value) -> String {
    let parts: Vec<String> = result["content"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|part| match part["type"].as_str() {
            Some("text") => part["text"].as_str().unwrap_or_default().to_owned(),
            Some(other) => format!("[{other}]"),
            None => String::new(),
        })
        .collect();
    let text = parts.join("\n");
    let text = if result["isError"].as_bool().unwrap_or(false) {
        format!("error: {text}")
    } else {
        text
    };
    crate::core::text::truncate_chars(&text, MAX_RESULT_CHARS)
}

/// Function name the model sees for a server's tool: `mcp__<server>__<tool>`, limited to what chat APIs accept.
pub fn function_name(server: &str, tool: &str) -> String {
    let clean: String = format!("mcp__{server}__{tool}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    clean.chars().take(64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_names_are_safe_and_at_most_64_chars() {
        assert_eq!(
            function_name("codegraph", "codegraph_explore"),
            "mcp__codegraph__codegraph_explore"
        );
        assert_eq!(function_name("a", "x.y z"), "mcp__a__x_y_z");
        assert_eq!(function_name("a", &"t".repeat(100)).len(), 64);
    }

    #[test]
    fn tool_results_join_text_flag_errors_and_cap_length() {
        let ok = json!({"content":[{"type":"text","text":"a"},{"type":"image","data":"x"},{"type":"text","text":"b"}]});
        assert_eq!(result_text(&ok), "a\n[image]\nb");
        let bad = json!({"isError":true,"content":[{"type":"text","text":"boom"}]});
        assert_eq!(result_text(&bad), "error: boom");
        let long = json!({"content":[{"type":"text","text":"z".repeat(MAX_RESULT_CHARS * 2)}]});
        assert!(result_text(&long).chars().count() <= MAX_RESULT_CHARS + 20);
    }

    #[test]
    fn tools_keep_schema_and_read_only_hint() {
        let tool = parse_tool(&json!({"name":"t","description":"d","inputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}})).unwrap();
        assert!(tool.read_only && tool.schema["type"] == "object");
        let bare = parse_tool(&json!({"name":"u"})).unwrap();
        assert!(!bare.read_only && bare.schema["type"] == "object");
        assert!(parse_tool(&json!({"description":"no name"})).is_none());
    }
}
