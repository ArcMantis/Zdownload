<p align="right">
  <a href="./readme_CN.md">简体中文</a> | <b>English</b>
</p>

# 🚀 Zdownload - Cross-platform Video Downloader

A lightweight video download frontend built with Rust and the Slint framework. This project follows a minimalist system philosophy, deeply optimized for Debian 13 (Trixie) and GNOME 48, while supporting cross-compilation from Linux to Windows.

---

## 🌐 多语言界面展示 / UI Preview

| 🇨🇳 简体中文界面 | 🇺🇸 English Interface |
| :---: | :---: |
| ![CN](./docs/Preview_CN.png) | ![EN](./docs/Preview_EN.png) |
| *支持原生中文显示* | *Full English Support* |

---

## ✨ Key Features

* 🦀 Rust Powered: High performance, memory safety, and extremely low system resource usage.
* 🎨 Modern UI: Declarative UI via Slint, fully compatible with both Wayland and X11.
* 🛡️ Zero Runtime Dependencies: Static linking strategy ensures it runs without pre-installed OpenSSL or graphics development libraries.
* 📦 Cross-platform Support: Single codebase for both native Linux builds and Windows cross-compilation.

---

## 🐧 Linux Build Guide (Debian/Ubuntu)

Following the system minimalism principle, only core build dependencies are required for compilation.

### 1. Install Development Dependencies
Low-level development libraries for GUI rendering and secure transmission (~18MB):

```bash
sudo apt update
sudo apt install -y --no-install-recommends build-essential pkg-config libwayland-dev libxkbcommon-dev libfontconfig1-dev libssl-dev
```

### 2. Configure Rust Environment
If the Rust toolchain is not yet installed, run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. Compile Production Build
Enable LTO optimization and symbol stripping for peak performance and minimal file size:

```bash
git clone https://github.com/ArcMantis/Zdownload.git
cd Zdownload
cargo build --release
```

> Output Path: target/release/zdownloadwin

---

## 🪟 Windows Cross-Compilation

Build a standalone Windows portable version (.exe) directly on Debian:

### 1. Add Build Target and Toolchain
```bash
rustup target add x86_64-pc-windows-gnu
sudo apt update && sudo apt install -y binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64
```

### 2. Execute Cross-build
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

---

## 📦 Running & Compatibility

### Run Directly (Linux)
```bash
chmod +x ./target/release/zdownloadwin
./target/release/zdownloadwin
```

### Compatibility Verification
Deeply verified via ldd, the generated binary only links to core Linux libraries (libc, libfontconfig, libz, etc.), making it ready to use in the following environments:
* **Debian 12/13+**
* **Ubuntu 22.04/24.04+**
* **Arch Linux / Fedora / openSUSE**

---

## 🛠️ Build Optimization (Cargo.toml)

The project is pre-configured with the following Release optimizations:

```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Enable Link Time Optimization
strip = true         # Automatically strip symbols
```
---

## 📔Acknowledgments
- Video Engine: [yt-dlp](https://github.com/yt-dlp/yt-dlp)
---

## 📄 License
This project is licensed under the [GNU GPL v3.0 license](LICENSE).
