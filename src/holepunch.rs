//! Holepunch (互联网远程打洞与 PSN 设备列表)。

use std::ffi::{CString, c_char};
use std::marker::PhantomData;
use std::ptr;

use crate::error::{Error, cvt};
use crate::ffi;
use crate::log::Log;
use crate::util::cchar_array_to_string;

pub type HolepunchConsoleType = ffi::ChiakiHolepunchConsoleType;
pub type HolepunchPortType = ffi::ChiakiHolepunchPortType;

/// Holepunch 设备信息 (拥有所有权)。
#[derive(Debug, Clone)]
pub struct HolepunchDeviceInfo {
    pub console_type: HolepunchConsoleType,
    pub device_name: String,
    pub device_uid: [u8; 32],
    pub remoteplay_enabled: bool,
}

/// 列出 PSN 账号下可远程的设备。
pub fn holepunch_list_devices(
    psn_oauth2_token: &str,
    console_type: HolepunchConsoleType,
    log: &Log,
) -> Result<Vec<HolepunchDeviceInfo>, Error> {
    let token = CString::new(psn_oauth2_token)
        .map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA))?;
    let mut devices: *mut ffi::ChiakiHolepunchDeviceInfo = ptr::null_mut();
    let mut count: usize = 0;
    cvt(unsafe {
        ffi::chiaki_holepunch_list_devices(
            token.as_ptr(),
            console_type,
            &mut devices,
            &mut count,
            log.as_ptr() as *mut _,
        )
    })?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let d = unsafe { &*devices.add(i) };
        out.push(HolepunchDeviceInfo {
            console_type: d.type_,
            device_name: cchar_array_to_string(&d.device_name),
            device_uid: d.device_uid,
            remoteplay_enabled: d.remoteplay_enabled,
        });
    }
    unsafe { ffi::chiaki_holepunch_free_device_list(&mut devices) };
    Ok(out)
}

/// 生成客户端设备 UID (DUID)。
pub fn holepunch_generate_client_device_uid() -> Result<String, Error> {
    let mut buf = [0 as c_char; 49];
    let mut size = buf.len();
    cvt(unsafe { ffi::chiaki_holepunch_generate_client_device_uid(buf.as_mut_ptr(), &mut size) })?;
    Ok(cchar_array_to_string(&buf[..size.min(buf.len())]))
}

/// Holepunch 会话句柄。`Drop` 时 fini (流结束后调用)。
pub struct HolepunchSession<'a> {
    raw: ffi::ChiakiHolepunchSession,
    _log: PhantomData<&'a Log>,
}

// SAFETY: 句柄为不透明指针, C 侧自同步, 方法全是 &mut。
unsafe impl Send for HolepunchSession<'_> {}

impl<'a> HolepunchSession<'a> {
    pub fn open(psn_oauth2_token: &str, log: &'a Log) -> Result<Self, Error> {
        let token = CString::new(psn_oauth2_token)
            .map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA))?;
        let raw =
            unsafe { ffi::chiaki_holepunch_session_init(token.as_ptr(), log.as_ptr() as *mut _) };
        if raw.is_null() {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_UNKNOWN));
        }
        Ok(HolepunchSession {
            raw,
            _log: PhantomData,
        })
    }

    pub fn create(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_holepunch_session_create(self.raw) })
    }

    pub fn start(
        &mut self,
        console_uid: &[u8; 32],
        console_type: HolepunchConsoleType,
    ) -> Result<(), Error> {
        cvt(unsafe {
            ffi::chiaki_holepunch_session_start(self.raw, console_uid.as_ptr(), console_type)
        })
    }

    pub fn punch_hole(&mut self, port_type: HolepunchPortType) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_holepunch_session_punch_hole(self.raw, port_type) })
    }

    pub fn upnp_discover(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_holepunch_upnp_discover(self.raw) })
    }

    pub fn create_offer(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::holepunch_session_create_offer(self.raw) })
    }

    pub fn force_port_guessing(&mut self, enabled: bool) {
        unsafe { ffi::chiaki_holepunch_session_force_port_guessing(self.raw, enabled) };
    }

    pub fn set_port_guessing_ports(&mut self, count: i32) {
        unsafe { ffi::chiaki_holepunch_session_set_port_guessing_ports(self.raw, count) };
    }

    pub fn set_port_guessing_socks(&mut self, count: i32) {
        unsafe { ffi::chiaki_holepunch_session_set_port_guessing_socks(self.raw, count) };
    }

    /// 打洞完成后的 regist 信息。
    pub fn regist_info(&mut self) -> ffi::ChiakiHolepunchRegistInfo {
        unsafe { ffi::chiaki_get_regist_info(self.raw) }
    }

    pub fn ps_addr(&mut self) -> String {
        let mut buf = [0 as c_char; 64];
        unsafe { ffi::chiaki_get_ps_selected_addr(self.raw, buf.as_mut_ptr()) };
        cchar_array_to_string(&buf)
    }

    pub fn ps_ctrl_port(&mut self) -> u16 {
        unsafe { ffi::chiaki_get_ps_ctrl_port(self.raw) }
    }

    /// 取打洞后的 socket 值 (由 session 管理, 不要 close)。
    pub fn socket(&mut self, port_type: HolepunchPortType) -> Result<ffi::chiaki_socket_t, Error> {
        let p = unsafe { ffi::chiaki_get_holepunch_sock(self.raw, port_type) };
        if p.is_null() {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_UNKNOWN));
        }
        Ok(unsafe { *p })
    }

    pub fn stun_allocation(&mut self) -> Option<(i32, bool)> {
        let mut inc = 0i32;
        let mut random = false;
        let ok =
            unsafe { ffi::chiaki_holepunch_session_get_stun_allocation(self.raw, &mut inc, &mut random) };
        if ok { Some((inc, random)) } else { None }
    }

    /// 取消建连流程 (stop_thread 控制是否停 websocket 线程)。
    pub fn cancel(&mut self, stop_thread: bool) {
        unsafe { ffi::chiaki_holepunch_main_thread_cancel(self.raw, stop_thread) };
    }
}

impl Drop for HolepunchSession<'_> {
    fn drop(&mut self) {
        unsafe { ffi::chiaki_holepunch_session_fini(self.raw) };
    }
}
