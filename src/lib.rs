//! Safe Rust bindings for `libchiaki` (the core library of chiaki-ng).
//!
//! 布局说明: 底层的 [`ffi`] 模块由 bindgen 从 C 头文件生成, 链接
//! `LIBCHIAKI_PREFIX` 指向的预编译静态库 (见 scripts/build-chiaki.sh)。
//! 各子模块在此之上提供 RAII 封装与闭包回调。内部实现细节结构体
//! (Session / Takion / Mutex 等) 的 bindgen 布局不可靠,
//! 因此大对象的分配与字段设置走 C 垫片 (shim/chiaki_shim.c),
//! Rust 侧只持有指针, 绝不直接实例化它们。
//!
//! Windows 上必须用 GNU target 构建:
//! `cargo build --target x86_64-pc-windows-gnu` (见 build.rs 的 panic 提示)。

#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

pub mod ffi;

mod shim;
mod util;

pub mod common;
pub mod connect;
pub mod controller;
pub mod discovery;
pub mod error;
pub mod feedback;
pub mod holepunch;
pub mod log;
pub mod regist;
pub mod session;

#[cfg(test)]
mod tests;

// 根导出保持扁平, 与拆分前一致。
pub use common::*;
pub use connect::*;
pub use controller::*;
pub use discovery::*;
pub use error::Error;
pub use feedback::*;
pub use holepunch::*;
pub use log::*;
pub use regist::*;
pub use session::*;
