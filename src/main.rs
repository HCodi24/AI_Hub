mod chat;
mod config;
mod extensions;
mod mutis;
mod plans;
mod providers;
mod server;

use anyhow::{Context, Result};
use std::net::SocketAddr;

#[derive(Debug)]
struct Options {
    port: u16,
    open_browser: bool,
}

fn parse_options() -> Result<Options> {
    let mut port = std::env::var("AI_HUB_PORT")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .context("AI_HUB_PORT must be a valid port")
        })
        .transpose()?
        .unwrap_or(7878);
    let mut open_browser = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                let value = args.next().context("--port requires a value")?;
                port = value.parse().context("--port must be a valid port")?;
            }
            "--no-open" => open_browser = false,
            "--help" | "-h" => {
                println!("AI Hub\n\nUsage: ai-hub [--port PORT] [--no-open]\n\nAI_HUB_PORT sets the default port (7878).");
                std::process::exit(0);
            }
            unknown => anyhow::bail!("Unknown argument: {unknown}"),
        }
    }
    Ok(Options { port, open_browser })
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = parse_options()?;
    let config_path = config::config_file_path()?;
    let config = config::load_or_seed(&config_path)?;
    let chat_dir = chat::config_chat_dir(&config_path)?;
    let state = server::AppState::new(config, config_path, chat_dir);

    let address = SocketAddr::from(([127, 0, 0, 1], options.port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("Could not bind to {address}"))?;
    let actual_port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{actual_port}/");
    println!("AI Hub is running at {url}");
    if options.open_browser {
        if let Err(error) = open::that(&url) {
            eprintln!("Could not open the browser: {error}");
        }
    }

    axum::serve(listener, server::router(state, actual_port))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("HTTP server failed")
}
