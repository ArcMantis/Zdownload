slint::include_modules!();
use std::fs;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let ui_handle = ui.as_weak();

    // 线程安全地持有当前下载进程
    let current_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    // --- 0. 启动初始化：尝试加载默认 Cookies ---
    if let Some(config_dir) = dirs::config_dir() {
        let default_path = config_dir.join("Zdownload").join("cookies.txt");
        if default_path.exists() {
            // 修正点：使用 .to_string() 确保类型匹配
            ui.set_cookie_path(default_path.to_string_lossy().to_string().into());
            append_log(
                &ui,
                &format!("✅ 已自动加载默认 Cookies: {}", default_path.display()),
            );
        }
    }

    // --- 1. UI 基础回调 ---
    ui.on_select_cookie_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("txt", &["txt"])
                .pick_file()
            {
                if let Some(ui) = ui_handle.upgrade() {
                    // 修正点：使用 .to_string() 确保类型匹配
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
                append_log(&ui, "Cookie 路径已从当前任务中移除。");
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

    // --- 2. 核心下载回调 ---
    ui.on_download_clicked({
        let ui_handle = ui_handle.clone();
        let proc_arc = current_process.clone();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let url = ui.get_url().trim().to_string();
            let cookie = ui.get_cookie_path().to_string();
            let quality_idx = ui.get_selected_quality_idx();

            if url.is_empty() {
                append_log(&ui, "❌ 错误: URL 不能为空。");
                return;
            }

            ui.set_downloading(true);
            append_log(&ui, "--- 准备下载任务 ---");

            // 画质映射
            let format_arg = match quality_idx {
                0 => "bestvideo[height<=2160]+bestaudio/best",
                1 => "bestvideo[height<=1080]+bestaudio/best",
                2 => "bestvideo[height<=720]+bestaudio/best",
                _ => "bestvideo[height<=480]+bestaudio/best",
            };

            let ui_thread = ui_handle.clone();
            let proc_thread = proc_arc.clone();

            tokio::spawn(async move {
                // 确定 yt-dlp 路径 (~/.local/share/zdownload/yt-dlp)
                let bin_dir = dirs::data_dir().unwrap_or_default().join("zdownload");
                let yt_dlp_path = bin_dir.join("yt-dlp");

                // 检查组件
                if !yt_dlp_path.exists() {
                    let _ = fs::create_dir_all(&bin_dir);
                    update_ui_log(&ui_thread, "📦 正在下载 yt-dlp 组件...");
                    let _ = Command::new("curl")
                        .args(&[
                            "-L",
                            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
                            "-o",
                        ])
                        .arg(&yt_dlp_path)
                        .status()
                        .await;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ =
                            fs::set_permissions(&yt_dlp_path, fs::Permissions::from_mode(0o755));
                    }
                }

                let download_dir = dirs::download_dir()
                    .unwrap_or_else(|| dirs::home_dir().unwrap().join("Downloads"));

                let mut args = vec![
                    "--newline".into(),
                    "--progress".into(),
                    "-f".into(),
                    format_arg.into(),
                    "-o".into(),
                    format!("{}/%(title)s.%(ext)s", download_dir.display()),
                    url,
                ];

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
                    update_ui_log(&ui_thread, "❌ 无法启动下载进程。");
                }

                // 结束清理
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_thread.upgrade() {
                        ui.set_downloading(false);
                        append_log(&ui, "✅ 任务执行完毕。");
                    }
                });
                let mut lock = proc_thread.lock().unwrap();
                *lock = None;
            });
        }
    });

    // --- 3. 取消下载回调 ---
    ui.on_cancel_clicked({
        let proc_arc = current_process.clone();
        let ui_handle = ui_handle.clone();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let mut lock = proc_arc.lock().unwrap();

            if let Some(mut child) = lock.take() {
                let ui_thread = ui_handle.clone();
                append_log(&ui, "⏳ 正在强制停止进程，请稍候...");

                // 开启异步任务等待进程彻底死亡
                tokio::spawn(async move {
                    let _ = child.start_kill(); // 发送 kill 信号
                    let _ = child.wait().await; // 关键：等待进程完全退出

                    // 进程消失后，在 UI 线程通知用户
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_thread.upgrade() {
                            ui.set_downloading(false);
                            append_log(&ui, "🛑 进程已完全退出，任务已取消。");
                        }
                    });
                });
            } else {
                append_log(&ui, "提示: 当前没有正在运行的任务。");
            }
        }
    });

    ui.run()
}

// 辅助函数：普通日志
fn append_log(ui: &MainWindow, text: &str) {
    let old = ui.get_log_text();
    ui.set_log_text(format!("{}{}\n", old, text).into());
}

// 辅助函数：跨线程日志
fn update_ui_log(ui_handle: &slint::Weak<MainWindow>, text: &'static str) {
    let handle = ui_handle.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle.upgrade() {
            append_log(&ui, text);
        }
    });
}

// 辅助函数：智能刷新进度
fn update_ui_log_smart(ui_handle: &slint::Weak<MainWindow>, new_line: &str) {
    let handle = ui_handle.clone();
    let line = new_line.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle.upgrade() {
            let mut log = ui.get_log_text().to_string();
            let trimmed = line.trim();
            // 匹配进度行以实现原地刷新
            if (trimmed.starts_with("[download]") || trimmed.starts_with("[抽取]"))
                && trimmed.contains('%')
                && !trimmed.contains("100%")
            {
                if let Some(pos) = log.trim_end().rfind('\n') {
                    if log[pos + 1..].trim().starts_with("[download]")
                        || log[pos + 1..].trim().starts_with("[抽取]")
                    {
                        log.truncate(pos + 1);
                    }
                }
            }
            ui.set_log_text(format!("{}{}\n", log, trimmed).into());
        }
    });
}
