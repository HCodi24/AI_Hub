//! MutiS desktop window. It starts the `ai-hub` server (same folder, or `AI_HUB_SERVER`) on a free local port with
//! `--no-open`, shows its web UI in a WebView2 window, restarts the server if it stops, and stops it on exit.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
};

const SERVER_EXE: &str = if cfg!(windows) { "ai-hub.exe" } else { "ai-hub" };

struct Server {
    child: Mutex<Option<Child>>,
    quitting: AtomicBool,
}

/// `AI_HUB_SERVER`, else next to this exe, else a `target/{release,debug}` build in a parent folder (repo checkout).
fn server_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("AI_HUB_SERVER").map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join(SERVER_EXE);
    if beside.is_file() {
        return Some(beside);
    }
    dir.ancestors()
        .flat_map(|root| ["release", "debug"].map(|profile| root.join("target").join(profile).join(SERVER_EXE)))
        .find(|path| path.is_file())
}

/// Same port every launch, because the web UI keeps theme, draft and open chat in localStorage, which is per origin
/// (a random port would reset them each time). Falls back to a random free port if this one is taken.
const PREFERRED_PORT: u16 = 47878;

fn free_port() -> Option<u16> {
    TcpListener::bind(("127.0.0.1", PREFERRED_PORT))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
        .ok()?
        .local_addr()
        .ok()
        .map(|addr| addr.port())
}

fn spawn_server(path: &PathBuf, port: u16) -> std::io::Result<Child> {
    let mut command = Command::new(path);
    command
        .args(["--no-open", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Arguments after `--` on our own command line go to the server (for example `--dev`).
    command.args(std::env::args().skip(1).skip_while(|arg| arg != "--").skip(1));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.spawn()
}

fn wait_ready(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// Stop the server and the CLI processes it started (Codex / Claude runs).
fn stop_server(server: &Server) {
    server.quitting.store(true, Ordering::SeqCst);
    let Some(mut child) = server.child.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn start_error(app: &tauri::AppHandle, message: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let text = serde_json_string(message);
        let _ = window.eval(format!("window.showStartError && window.showStartError({text})"));
    }
}

/// Minimal JSON string escaping (avoids a serde dependency for one call).
fn serde_json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Start the server, point the window at it, then keep it alive: if it exits while the app is open it is started
/// again on the same port (at most 5 times per minute so a crash loop cannot spin forever).
fn run_server(app: tauri::AppHandle, server: Arc<Server>) {
    let Some(path) = server_path() else {
        start_error(&app, "Không tìm thấy ai-hub.exe. Đặt nó cùng thư mục với ứng dụng hoặc đặt biến AI_HUB_SERVER.");
        return;
    };
    let Some(port) = free_port() else {
        start_error(&app, "Không tìm được cổng trống trên máy.");
        return;
    };
    let url = format!("http://127.0.0.1:{port}/");
    let mut first = true;
    let mut restarts: Vec<Instant> = Vec::new();
    loop {
        if server.quitting.load(Ordering::SeqCst) {
            return;
        }
        match spawn_server(&path, port) {
            Ok(child) => *server.child.lock().unwrap() = Some(child),
            Err(error) => {
                start_error(&app, &format!("Không chạy được {}: {error}", path.display()));
                return;
            }
        }
        if !wait_ready(port, Duration::from_secs(30)) {
            start_error(&app, "MutiS không phản hồi sau 30 giây.");
        } else if let Some(window) = app.get_webview_window("main") {
            if first {
                if let Ok(target) = url.parse() {
                    let _ = window.navigate(target);
                }
                first = false;
            }
        }
        // Wait until the server process ends.
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if server.quitting.load(Ordering::SeqCst) {
                return;
            }
            let ended = match server.child.lock().unwrap().as_mut() {
                Some(child) => !matches!(child.try_wait(), Ok(None)),
                None => true,
            };
            if ended {
                break;
            }
        }
        // A dev "Build & khởi động lại" replaces the server process itself: give the new one a moment to take the port.
        if wait_ready(port, Duration::from_secs(8)) {
            *server.child.lock().unwrap() = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                if server.quitting.load(Ordering::SeqCst) {
                    return;
                }
                if !wait_ready(port, Duration::from_secs(2)) {
                    break;
                }
            }
        }
        restarts.retain(|at| at.elapsed() < Duration::from_secs(60));
        if restarts.len() >= 5 {
            start_error(&app, "MutiS dừng liên tục. Hãy mở lại ứng dụng.");
            return;
        }
        restarts.push(Instant::now());
    }
}

fn main() {
    let server = Arc::new(Server { child: Mutex::new(None), quitting: AtomicBool::new(false) });
    let setup_server = server.clone();
    let app = tauri::Builder::default()
        // A second launch only brings the existing window to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("MutiS")
                // No native title bar: the web UI draws its own window buttons in the top strip.
                .decorations(false)
                .inner_size(1280.0, 820.0)
                .min_inner_size(420.0, 560.0)
                // Links that leave the local app (docs, sign-in pages) open in the default browser.
                .on_navigation(|url| {
                    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "tauri.localhost"))
                        || url.scheme() == "tauri";
                    if !local {
                        let _ = open::that(url.as_str());
                    }
                    local
                })
                .build()?;
            let open_item = MenuItem::with_id(app, "open", "Mở MutiS", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            let mut tray = TrayIconBuilder::with_id("main").tooltip("MutiS").menu(&menu).on_menu_event(
                |app, event| match event.id().as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                },
            );
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            let handle = app.handle().clone();
            let server = setup_server.clone();
            std::thread::spawn(move || run_server(handle, server));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("could not start the MutiS window");
    app.run(move |_app, event| {
        if matches!(event, RunEvent::Exit) {
            stop_server(&server);
        }
    });
}
