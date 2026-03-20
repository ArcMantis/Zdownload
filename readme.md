# Zdownload – Cross-Platform Video Download Tool 🚀

A lightweight video-downloading frontend built with Rust + Slint.  
Supports development on Debian 13, and can be cross-compiled to Windows with a single command.

---

## 🛠️ Build Environment Setup (Debian 13)

Before compiling, make sure your system has the required Rust cross-compilation environment configured.

---

## 🔧 Build for Windows

```bash
# Build for Windows platform

# 1. Add the Windows compilation target
rustup target add x86_64-pc-windows-gnu

# 2. Install the MinGW cross-compilation toolchain
#    (required for handling Windows icons and resource files)
sudo apt update && sudo apt install binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64 -y

# 3. Cross-compile Windows Release build
cargo build --release --target x86_64-pc-windows-gnu
```

```bash
# Build for Linux platform

# 1. Debug build
cargo run

# 2. Release build
cargo build --release
```
