#![windows_subsystem = "windows"]
slint::include_modules!();
use std::fs;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use url::Url;

// 辅助函数：校验 URL 是否合法且为 HTTP/HTTPS
fn is_valid_url(input: &str) -> bool {
    match Url::parse(input) {
        Ok(url) => {
            let scheme = url.scheme();
            scheme == "http" || scheme == "https"
        }
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let version = env!("CARGO_PKG_VERSION");
    let ui = MainWindow::new()?;
    let ui_handle = ui.as_weak();
    let current_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    append_log(
        &ui,
        &format!("🚀 Zdownload v{} - 跨平台视频下载工具", version),
    );

    // --- 初始化：默认 Cookies ---
    if let Some(config_dir) = dirs::config_dir() {
        let default_path = config_dir.join("Zdownload").join("cookies.txt");
        if default_path.exists() {
            ui.set_cookie_path(default_path.to_string_lossy().to_string().into());
            append_log(
                &ui,
                &format!("✅ 已加载默认 Cookies: {}", default_path.display()),
            );
        }
    }

    // --- UI 回调 (保持原样) ---
    ui.on_select_cookie_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("txt", &["txt"])
                .pick_file()
            {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_cookie_path(file.to_string_lossy().to_string().into());
                }
            }
        }
    });

    ui.on_clear_cookie_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_cookie_path("未选择文件".into());
            }
        }
    });

    ui.on_clear_url_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_url("".into());
            }
        }
    });

    // --- 核心下载逻辑 ---
    ui.on_download_clicked({
        let ui_handle = ui_handle.clone();
        let proc_arc = current_process.clone();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let raw_url = ui.get_url().trim().to_string();

            if raw_url.is_empty() {
                append_log(&ui, "⚠️ 提示: 视频链接不能为空。");
                return;
            }
            if !is_valid_url(&raw_url) {
                append_log(&ui, "❌ 错误: 输入的链接格式非法。");
                return;
            }

            ui.set_downloading(true);
            append_log(&ui, "--- 任务开始 ---");

            let cookie = ui.get_cookie_path().to_string();
            let quality_idx = ui.get_selected_quality_idx();

            let (format_arg, is_audio_only) = match quality_idx {
                0 => ("bestvideo[height<=2160]+bestaudio/best", false),
                1 => ("bestvideo[height<=1080]+bestaudio/best", false),
                2 => ("bestvideo[height<=720]+bestaudio/best", false),
                3 => ("bestvideo[height<=480]+bestaudio/best", false),
                _ => ("bestaudio/best", true),
            };

            let ui_thread = ui_handle.clone();
            let proc_thread = proc_arc.clone();

            tokio::spawn(async move {
                // 1. 确定对应系统的文件名
                let bin_name = if cfg!(target_os = "windows") {
                    "yt-dlp.exe"
                } else {
                    "yt-dlp"
                };

                // 2. 环境探测：先找系统环境变量，再找私有目录
                let yt_dlp_path = if let Ok(p) = which::which(bin_name) {
                    p
                } else {
                    let bin_dir = dirs::data_dir().unwrap_or_default().join("zdownload");
                    let p = bin_dir.join(bin_name);

                    if !p.exists() {
                        let _ = fs::create_dir_all(&bin_dir);
                        update_ui_log_str(&ui_thread, "📦 正在获取下载引擎 (yt-dlp)...");

                        let url = if cfg!(target_os = "windows") {
                            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
                        } else {
                            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
                        };

                        // 使用 reqwest 下载，比 Command curl 更跨平台可靠
                        if let Ok(resp) = reqwest::get(url).await {
                            if let Ok(bytes) = resp.bytes().await {
                                if fs::write(&p, bytes).is_ok() {
                                    #[cfg(unix)]
                                    {
                                        use std::os::unix::fs::PermissionsExt;
                                        let _ = fs::set_permissions(
                                            &p,
                                            fs::Permissions::from_mode(0o755),
                                        );
                                    }
                                    update_ui_log_str(&ui_thread, "✅ 引擎下载成功。");
                                }
                            }
                        }
                    }
                    p
                };

                let download_dir = dirs::download_dir()
                    .unwrap_or_else(|| dirs::home_dir().unwrap().join("Downloads"));

                let mut args = vec![
                    "--newline".into(),
                    "--progress".into(),
                    "-f".into(),
                    format_arg.into(),
                    "-o".into(),
                    format!("{}/%(title)s.%(ext)s", download_dir.display()),
                ];

                if is_audio_only {
                    args.push("--extract-audio".into());
                    args.push("--audio-format".into());
                    args.push("mp3".into());
                }

                args.push(raw_url);
                if cookie != "未选择文件" {
                    args.push("--cookies".into());
                    args.push(cookie);
                }

                let mut cmd = Command::new(&yt_dlp_path);
                cmd.args(&args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Ok(mut child) = cmd.spawn() {
                    let stdout = child.stdout.take().unwrap();
                    {
                        let mut lock = proc_thread.lock().unwrap();
                        *lock = Some(child);
                    }

                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        update_ui_log_smart(&ui_thread, &line);
                    }
                } else {
                    update_ui_log_str(&ui_thread, "❌ 引擎启动失败，请检查是否安装了 ffmpeg。");
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_thread.upgrade() {
                        ui.set_downloading(false);
                        append_log(&ui, "✅ 任务结束。");
                    }
                });
                let mut lock = proc_thread.lock().unwrap();
                *lock = None;
            });
        }
    });

    ui.on_cancel_clicked({
        let proc_arc = current_process.clone();
        let ui_handle = ui_handle.clone();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let mut lock = proc_arc.lock().unwrap();
            if let Some(mut child) = lock.take() {
                append_log(&ui, "⏳ 正在强制停止...");
                let ui_thread = ui_handle.clone();
                tokio::spawn(async move {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_thread.upgrade() {
                            ui.set_downloading(false);
                            append_log(&ui, "🛑 任务已取消。");
                        }
                    });
                });
            }
        }
    });

    ui.run()
}

// --- 辅助函数 ---
fn append_log(ui: &MainWindow, text: &str) {
    let old = ui.get_log_text();
    ui.set_log_text(format!("{}{}\n", old, text).into());
}

fn update_ui_log_str(ui_handle: &slint::Weak<MainWindow>, text: &'static str) {
    let handle = ui_handle.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle.upgrade() {
            append_log(&ui, text);
        }
    });
}

fn update_ui_log_smart(ui_handle: &slint::Weak<MainWindow>, new_line: &str) {
    let handle = ui_handle.clone();
    let line = new_line.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle.upgrade() {
            let mut log = ui.get_log_text().to_string();
            let trimmed = line.trim();
            if (trimmed.starts_with("[download]") || trimmed.contains("%"))
                && !trimmed.contains("100%")
            {
                if let Some(pos) = log.trim_end().rfind('\n') {
                    if log[pos + 1..].trim().starts_with("[download]") {
                        log.truncate(pos + 1);
                    }
                }
            }
            ui.set_log_text(format!("{}{}\n", log, trimmed).into());
        }
    });
}
