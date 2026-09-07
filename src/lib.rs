//! Safe Rust bindings for `libchiaki` (the core library of chiaki-ng).
//!
//! 布局说明: 底层的 [`ffi`] 模块由 bindgen 从 `LIBCHIAKI_PREFIX` 指向的
//! 安装头文件生成 (见 build-libchiaki.sh), 链接其预编译静态库。
//! 所有类型布局直接取自 bindgen 对真实头文件的解析; chiaki 头文件里的
//! `static inline` 辅助函数由 bindgen 的 wrap_static_fns 生成 C 包装并
//! 在构建期编译 (build.rs)。若 bindgen 因前向声明把某个类型降级成
//! opaque 占位, 构建会直接失败 (build.rs 的 opaque 检查),
//! `cargo test` 还会运行 bindgen 生成的逐类型布局测试。
//!
//! Windows 上必须用 GNU target 构建:
//! `cargo build --target x86_64-pc-windows-gnu` (见 build.rs 的 panic 提示)。
//!
//! API 覆盖情况见 `docs/API_COVERAGE.md`。

#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

pub mod ffi;

mod util;

pub mod common;
pub mod connect;
pub mod controller;
pub mod discovery;
pub mod error;
pub mod feedback;
pub mod holepunch;
pub mod log;
#[cfg(feature = "opus")]
pub mod opus;
pub mod orientation;
pub mod regist;
pub mod session;
pub mod sock;

// 根导出保持扁平, 与拆分前一致。
pub use common::*;
pub use connect::*;
pub use controller::*;
pub use discovery::*;
pub use error::Error;
pub use feedback::*;
pub use holepunch::*;
pub use log::*;
#[cfg(feature = "opus")]
pub use opus::*;
pub use orientation::*;
pub use regist::*;
pub use session::*;
pub use sock::*;
