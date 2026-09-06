//! 日志 (`ChiakiLog` 的 RAII 封装)。

use std::ffi::CStr;
use std::marker::PhantomData;
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
            ffi::chiaki_log_init(&mut *raw, mask, Some(log_trampoline::<F>), holder.ptr);
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

impl std::ops::Deref for Log {
    type Target = ffi::ChiakiLog;
    fn deref(&self) -> &ffi::ChiakiLog {
        &self.raw
    }
}

/// C: `chiaki_log_sniffer_*` — 在原日志之前截流一份副本到内存缓冲。
/// 典型用法: session / regist 用 sniffer 的 log (`as_log_ptr` 或 Deref),
/// 结束后 `buffer()` 拿到全部输出展示在界面里。
pub struct LogSniffer<'a> {
    raw: Box<ffi::ChiakiLogSniffer>,
    _forward: PhantomData<&'a Log>,
}

// SAFETY: buf 由 chiaki 内部线程追加 (加锁), 结构体本身 init 后只读。
unsafe impl Send for LogSniffer<'_> {}
unsafe impl Sync for LogSniffer<'_> {}

impl<'a> LogSniffer<'a> {
    /// C: `chiaki_log_sniffer_init` — 截流 `mask` 级别的日志, 全部
    /// 转发给 `forward`。
    pub fn new(mask: u32, forward: &'a Log) -> Self {
        let mut raw = unsafe { crate::util::zeroed_box::<ffi::ChiakiLogSniffer>() };
        unsafe { ffi::chiaki_log_sniffer_init(&mut *raw, mask, forward.as_ptr() as *mut _) };
        LogSniffer {
            raw,
            _forward: PhantomData,
        }
    }

    /// C: `chiaki_log_sniffer_get_buffer` — 目前已截流的全部文本。
    pub fn buffer(&self) -> String {
        if self.raw.buf.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(self.raw.buf).to_string_lossy().into_owned() }
    }

    /// 嗅探到的日志指针 (交给 Session::new_with_raw_log 等)。
    /// SAFETY: C 侧只往 sniff_log 写 level_mask, 返回 *mut 以匹配 API。
    pub fn as_log_ptr(&self) -> *mut ffi::ChiakiLog {
        &self.raw.sniff_log as *const _ as *mut _
    }
}

impl std::ops::Deref for LogSniffer<'_> {
    type Target = ffi::ChiakiLog;
    fn deref(&self) -> &ffi::ChiakiLog {
        &self.raw.sniff_log
    }
}

impl Drop for LogSniffer<'_> {
    fn drop(&mut self) {
        unsafe { ffi::chiaki_log_sniffer_fini(&mut *self.raw) };
    }
}
