//! bindgen 生成的原始 FFI 绑定 (头文件 + 静态库见 build.rs)。
//!
//! 布局直接来自 bindgen 对安装头文件的解析; chiaki 头文件里的
//! `static inline` 辅助函数经 wrap_static_fns 生成 C 包装并由 build.rs
//! 编译链接 (带 __extern 后缀)。构建期的 opaque 占位检查 (build.rs) 与
//! bindgen 生成的布局测试 (cargo test) 保证布局可信。

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
