fn main() {
    // 告诉 Slint 编译你的 UI 文件
    slint_build::compile("src/app.slint").expect("Slint 编译失败");
}
