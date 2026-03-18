/*
时间线 →
─────────────────────────────────────────────────

第一步：class_init
    ↓
第二步：instance_init（创建所有UI控件）
    ↓
第三步：属性设置（GLib内部）
    ↓
第四步：parent_constructed（父类初始化）
    ↓
第五步：setup_callbacks ← 你的代码在这里
    ↓
第六步：窗口显示
    ↓
第七步：用户点击按钮 → 触发回调
*/

use crate::config::VERSION;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib, CompositeTemplate};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread; // 新增：用于路径处理

mod imp {
    use super::*; //导入父类的全部内容

    //自动实现某些trait 可以打印调试信息  可以创建默认值   可以从UI模板加载控件 /  指定模板路径
    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/zzm/Zdownload/window.ui")]
    pub struct ZdownloadWindow {
        #[template_child]
        pub url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub clear_url_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cookie_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub cookie_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub clear_cookie_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub quality_combo: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub download_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub log_view: TemplateChild<gtk::TextView>,

        // 线程安全地持有当前正在运行的下载进程
        pub current_process: Arc<Mutex<Option<Child>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ZdownloadWindow {
        const NAME: &'static str = "ZdownloadWindow";
        type Type = super::ZdownloadWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ZdownloadWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_callbacks();
        }
    }
    impl WidgetImpl for ZdownloadWindow {}
    impl WindowImpl for ZdownloadWindow {}
    impl ApplicationWindowImpl for ZdownloadWindow {}
    impl AdwApplicationWindowImpl for ZdownloadWindow {}
}

glib::wrapper! {
    pub struct ZdownloadWindow(ObjectSubclass<imp::ZdownloadWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ZdownloadWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    // 辅助函数：获取默认 Cookie 路径 (~/.config/Zdownload/cookies.txt)
    fn get_default_cookie_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("Zdownload");
        path.push("cookies.txt");
        path
    }

    fn setup_callbacks(&self) {
        let imp = self.imp(); //获取内部实现（可以访问所有UI控件）,在闭包中直接访问self会移动所有权，但我们需要多次使用

        // --- 最终修复：直接获取 Display 且不使用 if let ---
        let provider = gtk::CssProvider::new();
        provider
            .load_from_string("textview { font-family: 'Noto Sans CJK SC'; font-size: 12pt; }");

        // 获取当前控件关联的 display
        let display = imp.log_view.display();

        // 直接应用到该 display 上
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        // ----------------------------------------------

        let window = self; //保存self的引用，方便在闭包中使用
        let default_cookie_hint = "请选择 .txt 格式的 cookies 文件";
        self.append_log(&format!("Version: {}", VERSION));

        // 初始化 UI 状态
        imp.cancel_button.set_label("取消下载");

        // --- 0. 启动时自动检查默认 Cookies ---
        let default_cookie = Self::get_default_cookie_path();
        if default_cookie.exists() {
            let path_str = default_cookie.display().to_string();
            imp.cookie_row.set_subtitle(&path_str); //pub cookie_row: TemplateChild<adw::ActionRow>,是一个显示标题和副标题的框框
            imp.clear_cookie_button.set_visible(true);
            self.append_log(&format!("已自动加载默认 Cookies: {}", path_str));
        } else {
            self.append_log(&format!(
                "请把默认cookies放入到下载目录然后手动选取,\n或者直接把cookies放到{}自动加载",
                default_cookie.display().to_string()
            ));
        }

        // --- 1. URL 输入框交互逻辑 ---
        imp.url_entry.connect_changed(glib::clone!(
            #[weak]
            imp,
            move |entry| {
                imp.clear_url_button.set_visible(!entry.text().is_empty());
            }
        ));

        imp.clear_url_button.connect_clicked(glib::clone!(
            #[weak]
            imp,
            move |_| {
                imp.url_entry.set_text("");
                imp.url_entry.grab_focus();
            }
        ));

        // --- 2. Cookies 选择与重置逻辑 ---
        imp.cookie_button.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| window.select_cookies_file()
        ));

        imp.clear_cookie_button.connect_clicked(glib::clone!(
            #[weak] window, #[weak] imp, move |_| {
                imp.cookie_row.set_subtitle(default_cookie_hint);
                imp.clear_cookie_button.set_visible(false);
                window.append_log("Cookie 路径已从当前任务中移除。");
                window.append_log(&format!("\n\n请把默认cookies放入到download目录然后手动选取,\n或者直接把cookies放到{}自动加载\n\n",default_cookie.display().to_string()));
            }
        ));

        // --- 3. 下载按钮逻辑 ---
        imp.download_button.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| {
                let url = window.imp().url_entry.text().to_string();
                if url.is_empty() {
                    window.append_log("错误: URL 不能为空。");
                    return;
                }

                // 此时 cookie_subtitle 可能是默认路径，也可能是用户选的路径
                let cookie_subtitle = window
                    .imp()
                    .cookie_row
                    .subtitle()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                window.imp().download_button.set_sensitive(false);
                window.append_log("--- 准备下载 ---");

                let (tx, rx) = mpsc::channel::<String>();
                let window_weak = window.downgrade();
                let process_arc = window.imp().current_process.clone();

                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    let window_ref = match window_weak.upgrade() {
                        Some(w) => w,
                        None => return glib::ControlFlow::Break,
                    };

                    while let Ok(msg) = rx.try_recv() {
                        window_ref.append_log(&msg);
                        if msg.contains("✅") || msg.contains("❌") || msg.contains("🛑") {
                            window_ref.imp().download_button.set_sensitive(true);
                            return glib::ControlFlow::Break;
                        }
                    }
                    glib::ControlFlow::Continue
                });

                thread::spawn(move || {
                    let mut bin_dir = glib::user_data_dir();
                    bin_dir.push("zdownload");
                    if !bin_dir.exists() {
                        let _ = fs::create_dir_all(&bin_dir);
                    }
                    let yt_dlp_path = bin_dir.join("yt-dlp");

                    if !yt_dlp_path.exists() {
                        let _ = tx.send("组件不存在，正在下载 yt-dlp...".to_string());
                        let status = Command::new("curl")
                            .args(&[
                                "-L",
                                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
                                "-o",
                                yt_dlp_path.to_str().unwrap(),
                            ])
                            .status();

                        let download_success = if let Ok(s) = status {
                            s.success()
                        } else {
                            false
                        };
                        if download_success {
                            let _ = fs::set_permissions(
                                &yt_dlp_path,
                                fs::Permissions::from_mode(0o755),
                            );
                        } else {
                            let _ = tx.send("❌ 无法下载组件。".to_string());
                            return;
                        }
                    }

                    let download_path = glib::user_special_dir(glib::UserDirectory::Downloads)
                        .unwrap_or_else(|| glib::home_dir().join("Downloads"));

                    let mut args = vec![
                        "--newline".to_string(),
                        "--progress".to_string(),
                        "-o".to_string(),
                        format!("{}/%(title)s.%(ext)s", download_path.display()),
                        url,
                    ];

                    // 逻辑：如果 Subtitle 不是提示词且不为空，则作为 --cookies 传入
                    if !cookie_subtitle.is_empty() && cookie_subtitle != default_cookie_hint {
                        args.push("--cookies".to_string());
                        args.push(cookie_subtitle);
                    }

                    let mut cmd = Command::new(&yt_dlp_path);
                    cmd.args(&args)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());

                    if let Ok(child) = cmd.spawn() {
                        {
                            let mut lock = process_arc.lock().unwrap();
                            *lock = Some(child);
                        }

                        let stdout = {
                            let mut lock = process_arc.lock().unwrap();
                            lock.as_mut().and_then(|c| c.stdout.take())
                        };

                        if let Some(out) = stdout {
                            let reader = BufReader::new(out);
                            for line in reader.lines() {
                                if let Ok(l) = line {
                                    if !l.trim().is_empty() {
                                        let _ = tx.send(l);
                                    }
                                }
                            }
                        }

                        let mut lock = process_arc.lock().unwrap();
                        if let Some(mut c) = lock.take() {
                            let status = c.wait().unwrap();
                            if status.success() {
                                let _ = tx.send("✅ 任务已成功完成！".to_string());
                            } else {
                                let _ = tx.send("❌ 下载已停止或出错。".to_string());
                            }
                        }
                    } else {
                        let _ = tx.send("❌ 无法启动下载进程。".to_string());
                    }
                });
            }
        ));

        // --- 4. 取消下载逻辑 ---
        imp.cancel_button.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| {
                let mut lock = window.imp().current_process.lock().unwrap();
                if let Some(mut child) = lock.take() {
                    // 1. 发送杀死信号
                    let _ = child.kill();
                    window.append_log("🛑 正在停止下载任务...");

                    // 2. 关键：等待进程彻底退出
                    // 因为 kill 已经发出了，wait 会在这里停留直到 yt-dlp 进程完全消失
                    // 通常这个过程只有几毫秒到几十毫秒，不会卡死 UI
                    let _ = child.wait();

                    // 3. 此时进程已经不存在了，恢复 UI 是安全的
                    window.append_log("🛑 下载已彻底停止。");
                    window.imp().download_button.set_sensitive(true);
                } else {
                    window.append_log("提示: 当前没有正在运行的任务。");
                }
            }
        ));
    }

    //精简版日志打印
    // pub fn append_log(&self, text: &str) {
    //     let buffer = self.imp().log_view.buffer();
    //     let mut iter = buffer.end_iter();
    //     buffer.insert(&mut iter, &format!("{}\n", text));

    //     let mark = buffer.create_mark(None, &buffer.end_iter(), false);
    //     self.imp()
    //         .log_view
    //         .scroll_to_mark(&mark, 0.0, true, 0.0, 1.0);
    // }

    pub fn append_log(&self, text: &str) {
        let buffer = self.imp().log_view.buffer();
        let text_trimmed = text.trim();

        // 只有当这一行是 [download] 进度条，且不是最终完成(100%)或目的地(Destination)信息时，尝试“原地刷新”
        if text_trimmed.starts_with("[download]")
            && text_trimmed.contains("%")
            && !text_trimmed.contains("100%")
        {
            let mut last_line_start = buffer.end_iter();
            // 尝试回退到最后一行的开头
            if last_line_start.backward_line() {
                let mut last_line_end = buffer.end_iter();
                let last_line_content = buffer.text(&last_line_start, &last_line_end, false);

                // 如果最后一行也是下载进度，则将其删除，实现“原地替换”
                if last_line_content.trim().starts_with("[download]") {
                    buffer.delete(&mut last_line_start, &mut last_line_end);
                }
            }
        }

        // 正常插入新行（对于非进度日志，这里就是普通的追加）
        let mut iter = buffer.end_iter();
        buffer.insert(&mut iter, &format!("{}\n", text_trimmed));

        // 自动滚动到底部
        let mark = buffer.create_mark(None, &buffer.end_iter(), false);
        self.imp()
            .log_view
            .scroll_to_mark(&mark, 0.0, true, 0.0, 1.0);
    }

    fn select_cookies_file(&self) {
        let window = self;
        let dialog = gtk::FileDialog::builder()
            .title("选择 Cookies 文件")
            .build();
        dialog.open(
            Some(self),
            gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                window,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            let path_str = path.display().to_string();
                            window.imp().cookie_row.set_subtitle(&path_str);
                            window.imp().clear_cookie_button.set_visible(true);
                        }
                    }
                }
            ),
        );
    }
}
