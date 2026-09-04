//! 上线发现: Discovery / DiscoveryThread / DiscoveryService。

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::CString;
use std::marker::PhantomData;
use std::mem::size_of;
use std::os::raw::{c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Mutex;

use crate::error::{Error, cvt};
use crate::ffi;
use crate::log::Log;
use crate::shim;
use crate::util::{ErasedCallback, cstr_to_string, opt_cstr, zeroed_box};

#[cfg(windows)]
pub const AF_INET: u16 = 2;
#[cfg(windows)]
pub const AF_INET6: u16 = 23;
#[cfg(target_os = "linux")]
pub const AF_INET6: u16 = 10;
#[cfg(target_os = "macos")]
pub const AF_INET6: u16 = 30;
#[cfg(not(windows))]
pub const AF_INET: u16 = 2;

/// IPv4 socket 地址 (C ABI 稳定, 不依赖 bindgen)。
#[repr(C)]
struct SockAddrIn {
    family: u16,
    port_be: u16,
    addr: u32,
    zero: [u8; 8],
}

fn ipv4_sockaddr(ip: [u8; 4], port: u16) -> SockAddrIn {
    SockAddrIn {
        family: AF_INET,
        port_be: port.to_be(),
        addr: u32::from_ne_bytes(ip),
        zero: [0; 8],
    }
}

/// `ChiakiDiscoveryPacket` builder (拥有 protocol_version 字符串)。
pub struct DiscoveryPacket {
    raw: Box<ffi::ChiakiDiscoveryPacket>,
    _ver: CString,
}

// SAFETY: 裸指针指向自身拥有的 CString, 单一所有权。
unsafe impl Send for DiscoveryPacket {}

impl DiscoveryPacket {
    fn with_cmd(cmd: ffi::ChiakiDiscoveryCmd, version: &[u8], credential: u64) -> Self {
        // bindgen 的字符串宏是带 NUL 的字节数组, 先截断再 CString::new。
        let end = version.iter().position(|&b| b == 0).unwrap_or(version.len());
        let ver = CString::new(&version[..end]).expect("protocol version");
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiDiscoveryPacket>() };
        raw.cmd = cmd;
        raw.protocol_version = ver.as_ptr() as *mut c_char;
        raw.user_credential = credential;
        DiscoveryPacket { raw, _ver: ver }
    }

    pub fn new_search(ps5: bool) -> Self {
        let ver = if ps5 {
            ffi::CHIAKI_DISCOVERY_PROTOCOL_VERSION_PS5
        } else {
            ffi::CHIAKI_DISCOVERY_PROTOCOL_VERSION_PS4
        };
        Self::with_cmd(ffi::ChiakiDiscoveryCmd::CHIAKI_DISCOVERY_CMD_SRCH, ver, 0)
    }

    pub fn new_wakeup(credential: u64, ps5: bool) -> Self {
        let ver = if ps5 {
            ffi::CHIAKI_DISCOVERY_PROTOCOL_VERSION_PS5
        } else {
            ffi::CHIAKI_DISCOVERY_PROTOCOL_VERSION_PS4
        };
        Self::with_cmd(
            ffi::ChiakiDiscoveryCmd::CHIAKI_DISCOVERY_CMD_WAKEUP,
            ver,
            credential,
        )
    }

    pub fn format(&self) -> Result<String, Error> {
        let mut buf = [0 as c_char; 512];
        let n = unsafe {
            ffi::chiaki_discovery_packet_fmt(
                buf.as_mut_ptr(),
                buf.len(),
                &*self.raw as *const _ as *mut _,
            )
        };
        if n < 0 {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA));
        }
        let n = (n as usize).min(buf.len());
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, n) };
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// 发现到的主机信息 (拥有所有权)。
#[derive(Debug, Clone)]
pub struct DiscoveryHostInfo {
    pub state: ffi::ChiakiDiscoveryHostState,
    pub host_request_port: u16,
    pub host_addr: Option<String>,
    pub system_version: Option<String>,
    pub device_discovery_protocol_version: Option<String>,
    pub host_name: Option<String>,
    pub host_type: Option<String>,
    pub host_id: Option<String>,
    pub running_app_titleid: Option<String>,
    pub running_app_name: Option<String>,
    pub is_ps5: bool,
    pub target: crate::common::Target,
}

fn parse_discovery_host(h: &ffi::ChiakiDiscoveryHost) -> DiscoveryHostInfo {
    // is_ps5 / target 直接调 C 函数判定, 保证与库行为一致。
    let is_ps5 = unsafe { ffi::chiaki_discovery_host_is_ps5(h as *const _ as *mut _) };
    let target =
        unsafe { ffi::chiaki_discovery_host_system_version_target(h as *const _ as *mut _) };
    DiscoveryHostInfo {
        state: h.state,
        host_request_port: h.host_request_port,
        host_addr: opt_cstr(h.host_addr),
        system_version: opt_cstr(h.system_version),
        device_discovery_protocol_version: opt_cstr(h.device_discovery_protocol_version),
        host_name: opt_cstr(h.host_name),
        host_type: opt_cstr(h.host_type),
        host_id: opt_cstr(h.host_id),
        running_app_titleid: opt_cstr(h.running_app_titleid),
        running_app_name: opt_cstr(h.running_app_name),
        is_ps5,
        target,
    }
}

pub fn discovery_host_state_string(s: ffi::ChiakiDiscoveryHostState) -> String {
    cstr_to_string(unsafe { ffi::chiaki_discovery_host_state_string(s) })
}

unsafe extern "C" fn discovery_trampoline<F>(
    host: *mut ffi::ChiakiDiscoveryHost,
    user: *mut c_void,
) where
    F: FnMut(DiscoveryHostInfo) + Send + 'static,
{
    if host.is_null() || user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let info = parse_discovery_host(unsafe { &*host });
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(info)));
    }
}

/// `ChiakiDiscovery` 封装。
pub struct Discovery<'a> {
    raw: Box<ffi::ChiakiDiscovery>,
    _log: PhantomData<&'a Log>,
}

// SAFETY: 同 Session (C 侧自同步, 方法全是 &mut)。
unsafe impl Send for Discovery<'_> {}

impl<'a> Discovery<'a> {
    pub fn new(log: &'a Log, ipv6: bool) -> Result<Self, Error> {
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiDiscovery>() };
        cvt(unsafe {
            ffi::chiaki_discovery_init(
                &mut *raw,
                log.as_ptr() as *mut _,
                if ipv6 { AF_INET6 } else { AF_INET },
            )
        })?;
        Ok(Discovery {
            raw,
            _log: PhantomData,
        })
    }

    /// 发送发现/唤醒包到指定 IPv4 地址。
    pub fn send_to_ipv4(
        &self,
        packet: &DiscoveryPacket,
        ip: [u8; 4],
        port: u16,
    ) -> Result<(), Error> {
        let addr = ipv4_sockaddr(ip, port);
        cvt(unsafe {
            ffi::chiaki_discovery_send(
                &*self.raw as *const _ as *mut _,
                &*packet.raw as *const _ as *mut _,
                &addr as *const _ as *mut ffi::sockaddr,
                size_of::<SockAddrIn>(),
            )
        })
    }

    /// 快捷: 向子网广播发送 SRCH (PS4: 987, PS5: 9302)。
    pub fn broadcast_search(&self, packet: &DiscoveryPacket, ps5: bool) -> Result<(), Error> {
        let port = if ps5 {
            ffi::CHIAKI_DISCOVERY_PORT_PS5
        } else {
            ffi::CHIAKI_DISCOVERY_PORT_PS4
        } as u16;
        self.send_to_ipv4(packet, [255, 255, 255, 255], port)
    }

    /// `chiaki_discovery_wakeup` 快捷函数。
    pub fn wakeup(
        log: &Log,
        discovery: Option<&Discovery>,
        host: &str,
        user_credential: u64,
        ps5: bool,
    ) -> Result<(), Error> {
        let h =
            CString::new(host).map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA))?;
        cvt(unsafe {
            ffi::chiaki_discovery_wakeup(
                log.as_ptr() as *mut _,
                discovery
                    .map(|d| &*d.raw as *const _ as *mut _)
                    .unwrap_or(ptr::null_mut()),
                h.as_ptr(),
                user_credential,
                ps5,
            )
        })
    }
}

impl Drop for Discovery<'_> {
    fn drop(&mut self) {
        unsafe { ffi::chiaki_discovery_fini(&mut *self.raw) };
    }
}

/// 后台监听线程。`stop()` 会 join, `Drop` 也会 join。
pub struct DiscoveryThread<'a> {
    raw: Option<Box<ffi::ChiakiDiscoveryThread>>,
    _cb: ErasedCallback,
    _marker: PhantomData<&'a mut Discovery<'a>>,
}

// SAFETY: 同 Session。
unsafe impl Send for DiscoveryThread<'_> {}

impl<'a> DiscoveryThread<'a> {
    pub fn start<F>(
        discovery: &'a mut Discovery<'a>,
        oneshot: bool,
        cb: F,
    ) -> Result<Self, Error>
    where
        F: FnMut(DiscoveryHostInfo) + Send + 'static,
    {
        let holder = ErasedCallback::new(cb);
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiDiscoveryThread>() };
        let rc = unsafe {
            if oneshot {
                ffi::chiaki_discovery_thread_start_oneshot(
                    &mut *raw,
                    &mut *discovery.raw,
                    Some(discovery_trampoline::<F>),
                    holder.ptr,
                )
            } else {
                ffi::chiaki_discovery_thread_start(
                    &mut *raw,
                    &mut *discovery.raw,
                    Some(discovery_trampoline::<F>),
                    holder.ptr,
                )
            }
        };
        cvt(rc)?;
        Ok(DiscoveryThread {
            raw: Some(raw),
            _cb: holder,
            _marker: PhantomData,
        })
    }

    /// 停止并 join (消费 self, 防止重复 join)。
    pub fn stop(mut self) -> Result<(), Error> {
        let mut raw = self.raw.take().expect("DiscoveryThread already stopped");
        cvt(unsafe { ffi::chiaki_discovery_thread_stop(&mut *raw) })
    }
}

impl Drop for DiscoveryThread<'_> {
    fn drop(&mut self) {
        if let Some(mut raw) = self.raw.take() {
            unsafe {
                let _ = ffi::chiaki_discovery_thread_stop(&mut *raw);
            }
        }
    }
}

/// `ChiakiDiscoveryServiceOptions` builder (拥有 send_host 字符串)。
pub struct DiscoveryServiceOptions {
    raw: Box<ffi::ChiakiDiscoveryServiceOptions>,
    _send_host: Option<CString>,
}

// SAFETY: 裸指针全空或指向自身拥有的 CString。
unsafe impl Send for DiscoveryServiceOptions {}

impl DiscoveryServiceOptions {
    pub fn new() -> Self {
        DiscoveryServiceOptions {
            raw: unsafe { zeroed_box::<ffi::ChiakiDiscoveryServiceOptions>() },
            _send_host: None,
        }
    }

    pub fn set_hosts_max(&mut self, n: usize) {
        self.raw.hosts_max = n;
    }
    pub fn set_ping_ms(&mut self, ms: u64) {
        self.raw.ping_ms = ms;
    }
    pub fn set_ping_initial_ms(&mut self, ms: u64) {
        self.raw.ping_initial_ms = ms;
    }
    pub fn set_host_drop_pings(&mut self, n: u64) {
        self.raw.host_drop_pings = n;
    }
    pub fn set_send_host(&mut self, host: &str) -> Result<(), std::ffi::NulError> {
        let c = CString::new(host)?;
        self.raw.send_host = c.as_ptr() as *mut c_char;
        self._send_host = Some(c);
        Ok(())
    }
}

impl Default for DiscoveryServiceOptions {
    fn default() -> Self {
        Self::new()
    }
}

unsafe extern "C" fn service_trampoline<F>(
    hosts: *mut ffi::ChiakiDiscoveryHost,
    hosts_count: usize,
    user: *mut c_void,
) where
    F: FnMut(Vec<DiscoveryHostInfo>) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let mut v = Vec::with_capacity(hosts_count);
    for i in 0..hosts_count {
        let h = unsafe { &*hosts.add(i) };
        v.push(parse_discovery_host(h));
    }
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(v)));
    }
}

/// `ChiakiDiscoveryService` 封装 (内存按 C 真实大小分配)。
pub struct DiscoveryService<'a> {
    ptr: *mut ffi::ChiakiDiscoveryService,
    layout: Layout,
    _options: DiscoveryServiceOptions,
    _cb: ErasedCallback,
    _log: PhantomData<&'a Log>,
}

// SAFETY: 同 Session。
unsafe impl Send for DiscoveryService<'_> {}

impl<'a> DiscoveryService<'a> {
    pub fn new<F>(
        options: DiscoveryServiceOptions,
        log: &'a Log,
        cb: F,
    ) -> Result<Self, Error>
    where
        F: FnMut(Vec<DiscoveryHostInfo>) + Send + 'static,
    {
        let holder = ErasedCallback::new(cb);
        let size = unsafe { shim::libchiaki_sizeof_ChiakiDiscoveryService() };
        let align = unsafe { shim::libchiaki_alignof_ChiakiDiscoveryService() };
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_UNKNOWN))?;
        let ptr = unsafe { alloc(layout) as *mut ffi::ChiakiDiscoveryService };
        if ptr.is_null() {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_MEMORY));
        }
        unsafe { ptr::write_bytes(ptr as *mut u8, 0, size) };
        // 注意: service 会拷贝整个 options 结构体 (含 send_host 指针),
        // 因此 Service 必须拥有 options。
        let mut options = options;
        options.raw.cb = Some(service_trampoline::<F>);
        options.raw.cb_user = holder.ptr;
        let rc = unsafe {
            ffi::chiaki_discovery_service_init(ptr, &mut *options.raw, log.as_ptr() as *mut _)
        };
        if let Err(e) = cvt(rc) {
            unsafe { dealloc(ptr as *mut u8, layout) };
            return Err(e);
        }
        Ok(DiscoveryService {
            ptr,
            layout,
            _options: options,
            _cb: holder,
            _log: PhantomData,
        })
    }
}

impl Drop for DiscoveryService<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::chiaki_discovery_service_fini(self.ptr);
            dealloc(self.ptr as *mut u8, self.layout);
        }
    }
}
