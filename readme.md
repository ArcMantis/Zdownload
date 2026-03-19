# Zdownload - 跨平台视频下载工具 🚀

基于 Rust + Slint 框架开发的轻量级视频下载前端。支持在 Debian 13 环境下开发，并可一键交叉编译至 Windows。

---

## 🛠️ 构建环境准备 (Debian 13)

在开始编译前，请确保您的系统已配置好 Rust 交叉编译环境：

```bash
# 编译windows平台
# 1. 添加 Windows 编译目标
rustup target add x86_64-pc-windows-gnu

# 2. 安装 MinGW 交叉编译链 (用于处理 Windows 图标和资源文件)
sudo apt update && sudo apt install binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64 -y

# 交叉编译 Windows Release 版本
cargo build --release --target x86_64-pc-windows-gnu

```

```bash
# 编译linux 平台
 # 1.debug版本
 cargo run

 # 2.release 版本
 cargo build --release
```