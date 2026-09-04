//! bindgen 生成的原始 FFI 绑定 (头文件 + 静态库见 build.rs)。
//!
//! 注意: 其中部分内部结构体的布局是占位 opaque / 错误的
//! (ChiakiSession / ChiakiTakion / ChiakiMutex / nanopb 内部等),
//! 不要直接实例化它们, 用 crate 根的安全封装
//! (大对象走 C 垫片分配, 见 shim/chiaki_shim.c)。

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
