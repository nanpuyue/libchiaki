//! 内部小工具: C 字符串转换、清零分配、类型擦除回调持有者。
//!
//! 回调设计: 每个回调种类有一个泛型 trampoline (按闭包类型单态化),
//! user 指针指向堆上的定长 `Box<Mutex<F>>`, C 侧只透传。
//! `ErasedCallback` 拥有该 Box, 随属主对象一起释放
//! (属主 Drop 时先停掉 C 线程, 不存在 UAF)。

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::sync::Mutex;

/// 可空 C 字符串指针转 String (null 返回空串)。
pub(crate) fn cstr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

/// 可空 C 字符串指针转 Option<String>。
pub(crate) fn opt_cstr(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(cstr_to_string(p))
    }
}

/// 定长 c_char 数组转 String (按首个 NUL 截断, 无 NUL 则取全部)。
pub(crate) fn cchar_array_to_string(buf: &[c_char]) -> String {
    let bytes: &[u8] = unsafe { &*(std::ptr::from_ref(buf) as *const [u8]) };
    match CStr::from_bytes_until_nul(bytes) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// C 结构体清零分配 (chiaki 的 init 函数都接受未初始化内存, 清零更稳妥)。
pub(crate) unsafe fn zeroed_box<T>() -> Box<T> {
    unsafe { Box::new(std::mem::zeroed()) }
}

/// 类型擦除的回调持有者。堆上是定长的 `Box<Mutex<F>>`,
/// user 指针指向它 (thin 指针, 可跨 FFI)。
pub(crate) struct ErasedCallback {
    pub(crate) ptr: *mut c_void,
    drop_fn: unsafe fn(*mut c_void),
}

impl ErasedCallback {
    pub(crate) fn new<F: Send + 'static>(f: F) -> Self {
        unsafe fn drop_box<F>(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut Mutex<F>) });
        }
        let b = Box::new(Mutex::new(f));
        ErasedCallback {
            ptr: Box::into_raw(b) as *mut c_void,
            drop_fn: drop_box::<F>,
        }
    }
}

impl Drop for ErasedCallback {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.ptr) };
    }
}
