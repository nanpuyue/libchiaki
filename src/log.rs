//! 日志 (`ChiakiLog` 的 RAII 封装)。

use std::os::raw::{c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Mutex;

use crate::ffi;
use crate::util::{ErasedCallback, cstr_to_string};

pub use ffi::ChiakiLogLevel as LogLevel;

/// `CHIAKI_LOG_ALL` 掩码。
pub const LOG_ALL: u32 = ffi::CHIAKI_LOG_ALL;

pub fn log_level_char(level: LogLevel) -> char {
    unsafe { ffi::chiaki_log_level_char(level) as u8 as char }
}

unsafe extern "C" fn log_trampoline<F>(
    level: ffi::ChiakiLogLevel,
    msg: *const c_char,
    user: *mut c_void,
) where
    F: FnMut(LogLevel, &str) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let text = cstr_to_string(msg);
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(level, &text)));
    }
}

/// `ChiakiLog` 的 RAII 封装。必须比所有使用它的对象活得长
/// (Session / Discovery 等通过生命周期参数强制这一点)。
pub struct Log {
    raw: Box<ffi::ChiakiLog>,
    _cb: Option<ErasedCallback>,
}

// SAFETY: C 侧只在 init 时写入结构体; 之后多线程只是读取并调用
// 经 Mutex 保护的回调。`set_level` 必须在共享前调用 (见其文档)。
unsafe impl Send for Log {}
unsafe impl Sync for Log {}

impl Log {
    /// 带自定义回调的日志。回调可能被 chiaki 内部线程调用,
    /// 因此要求 `Send`; 回调内不要再调用 chiaki 日志 (会死锁)。
    pub fn new<F>(mask: u32, cb: F) -> Self
    where
        F: FnMut(LogLevel, &str) + Send + 'static,
    {
        let holder = ErasedCallback::new(cb);
        let mut raw = unsafe { crate::util::zeroed_box::<ffi::ChiakiLog>() };
        unsafe {
            ffi::chiaki_log_init(
                &mut *raw,
                mask,
                Some(log_trampoline::<F>),
                holder.ptr,
            );
        }
        Log {
            raw,
            _cb: Some(holder),
        }
    }

    /// 直接打印到 stdout 的日志 (`chiaki_log_cb_print`)。
    pub fn print_to_stdout(mask: u32) -> Self {
        let mut raw = unsafe { crate::util::zeroed_box::<ffi::ChiakiLog>() };
        unsafe {
            ffi::chiaki_log_init(
                &mut *raw,
                mask,
                Some(ffi::chiaki_log_cb_print),
                ptr::null_mut(),
            );
        }
        Log { raw, _cb: None }
    }

    /// 修改等级掩码。只能在单线程阶段调用。
    pub fn set_level(&mut self, mask: u32) {
        unsafe { ffi::chiaki_log_set_level(&mut *self.raw, mask) };
    }

    pub fn as_ptr(&self) -> *const ffi::ChiakiLog {
        &*self.raw
    }

    pub fn as_mut_ptr(&mut self) -> *mut ffi::ChiakiLog {
        &mut *self.raw
    }
}
