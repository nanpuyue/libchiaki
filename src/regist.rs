//! 配对注册 (`ChiakiRegist`)。

use std::ffi::{CString, NulError};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use crate::common::Target;
use crate::error::{Error, cvt};
use crate::ffi;
use crate::log::Log;
use crate::session::RegisteredHost;
use crate::session::parse_registered_host;
use crate::util::{ErasedCallback, zeroed_box};

/// `ChiakiRegistInfo` builder。
pub struct RegistInfo {
    raw: Box<ffi::ChiakiRegistInfo>,
    _host: CString,
    _online_id: Option<CString>,
}

// SAFETY: 裸指针指向自身拥有的 CString / 空。
unsafe impl Send for RegistInfo {}

impl RegistInfo {
    pub fn new(target: Target, host: &str, pin: u32) -> Result<Self, NulError> {
        let host_c = CString::new(host)?;
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiRegistInfo>() };
        raw.target = target;
        raw.host = host_c.as_ptr();
        raw.pin = pin;
        Ok(RegistInfo {
            raw,
            _host: host_c,
            _online_id: None,
        })
    }

    pub fn set_broadcast(&mut self, v: bool) {
        self.raw.broadcast = v;
    }
    /// C 契约: `psn_online_id` 为 null 时回退使用 `psn_account_id`
    /// (regist.h)。空字符串不是 null, 会被 C 当作非空 online_id 走
    /// 错误路径 — 因此这里与 GUI (qmlbackend.cpp 对空值传 nullptr)
    /// 一致: 空串等价于不设置。
    pub fn set_psn_online_id(&mut self, id: &str) -> Result<(), NulError> {
        if id.is_empty() {
            self.raw.psn_online_id = std::ptr::null_mut();
            self._online_id = None;
            return Ok(());
        }
        let c = CString::new(id)?;
        self.raw.psn_online_id = c.as_ptr();
        self._online_id = Some(c);
        Ok(())
    }
    pub fn set_psn_account_id(&mut self, id: &[u8; 8]) {
        self.raw.psn_account_id = *id;
    }
    pub fn set_console_pin(&mut self, pin: u32) {
        self.raw.console_pin = pin;
    }
}

/// 注册结果事件。
#[derive(Debug, Clone)]
pub enum RegistEvent {
    FinishedCanceled,
    FinishedFailed,
    FinishedSuccess(RegisteredHost),
}

unsafe extern "C" fn regist_trampoline<F>(event: *mut ffi::ChiakiRegistEvent, user: *mut c_void)
where
    F: FnMut(RegistEvent) + Send + 'static,
{
    if event.is_null() || user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let ev = unsafe { &*event };
    let out = match ev.type_ {
        ffi::ChiakiRegistEventType::CHIAKI_REGIST_EVENT_TYPE_FINISHED_CANCELED => {
            RegistEvent::FinishedCanceled
        }
        ffi::ChiakiRegistEventType::CHIAKI_REGIST_EVENT_TYPE_FINISHED_FAILED => {
            RegistEvent::FinishedFailed
        }
        ffi::ChiakiRegistEventType::CHIAKI_REGIST_EVENT_TYPE_FINISHED_SUCCESS => {
            if ev.registered_host.is_null() {
                RegistEvent::FinishedFailed
            } else {
                RegistEvent::FinishedSuccess(parse_registered_host(unsafe { &*ev.registered_host }))
            }
        }
    };
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(out)));
    }
}

/// `ChiakiRegist` 封装。`stop()` 后线程结束, `Drop` 会 stop + fini。
pub struct Regist<'a> {
    raw: Box<ffi::ChiakiRegist>,
    _info: RegistInfo,
    _cb: ErasedCallback,
    _log: PhantomData<&'a Log>,
}

// SAFETY: 同 Session。
unsafe impl Send for Regist<'_> {}

impl<'a> Regist<'a> {
    pub fn start<F>(log: &'a Log, info: RegistInfo, cb: F) -> Result<Self, Error>
    where
        F: FnMut(RegistEvent) + Send + 'static,
    {
        let holder = ErasedCallback::new(cb);
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiRegist>() };
        cvt(unsafe {
            ffi::chiaki_regist_start(
                &mut *raw,
                log.as_ptr() as *mut _,
                &*info.raw as *const _,
                Some(regist_trampoline::<F>),
                holder.ptr,
            )
        })?;
        Ok(Regist {
            raw,
            _info: info,
            _cb: holder,
            _log: PhantomData,
        })
    }

    pub fn stop(&mut self) {
        unsafe { ffi::chiaki_regist_stop(&mut *self.raw) };
    }
}

impl Drop for Regist<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::chiaki_regist_stop(&mut *self.raw);
            ffi::chiaki_regist_fini(&mut *self.raw);
        }
    }
}
