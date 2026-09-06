//! 库初始化与 Target / Codec / QuitReason 等小杂项
//! (对应 chiaki/common.h 与 session.h 的纯函数部分)。

use std::ffi::{CString, NulError};

use crate::error::{Error, cvt};
use crate::ffi;
use crate::util::cstr_to_string;

/// `chiaki_lib_init()`: 使用库之前调用一次。
pub fn lib_init() -> Result<(), Error> {
    cvt(unsafe { ffi::chiaki_lib_init() })
}

pub type Target = ffi::ChiakiTarget;
pub type Codec = ffi::ChiakiCodec;
pub type QuitReason = ffi::ChiakiQuitReason;
pub type DualSenseIntensity = ffi::ChiakiDualSenseEffectIntensity;

// 这四个是 C 头文件里的 static inline, 走 wrap_static_fns 生成的
// __extern 包装而非 Rust 重写 — 单一事实来源, 上游改动不会漂移
// (审计 A4)。
pub fn target_is_unknown(t: Target) -> bool {
    unsafe { ffi::chiaki_target_is_unknown(t) }
}

pub fn target_is_ps5(t: Target) -> bool {
    unsafe { ffi::chiaki_target_is_ps5(t) }
}

pub fn codec_is_h265(c: Codec) -> bool {
    unsafe { ffi::chiaki_codec_is_h265(c) }
}

pub fn codec_is_hdr(c: Codec) -> bool {
    unsafe { ffi::chiaki_codec_is_hdr(c) }
}

pub fn codec_name(c: Codec) -> String {
    cstr_to_string(unsafe { ffi::chiaki_codec_name(c) })
}

pub fn rp_version_string(t: Target) -> Option<String> {
    let p = unsafe { ffi::chiaki_rp_version_string(t) };
    if p.is_null() {
        None
    } else {
        Some(cstr_to_string(p))
    }
}

pub fn rp_version_parse(s: &str, is_ps5: bool) -> Result<Target, NulError> {
    let c = CString::new(s)?;
    Ok(unsafe { ffi::chiaki_rp_version_parse(c.as_ptr(), is_ps5) })
}

pub fn rp_application_reason_string(reason: u32) -> String {
    cstr_to_string(unsafe { ffi::chiaki_rp_application_reason_string(reason) })
}

pub fn quit_reason_string(r: QuitReason) -> String {
    cstr_to_string(unsafe { ffi::chiaki_quit_reason_string(r) })
}

pub fn quit_reason_is_error(r: QuitReason) -> bool {
    unsafe { ffi::chiaki_quit_reason_is_error(r) }
}

/// C: `chiaki_error_string`。
pub fn error_string(code: ffi::ChiakiErrorCode) -> String {
    cstr_to_string(unsafe { ffi::chiaki_error_string(code) })
}

/// C: `chiaki_time_now_monotonic_us` — 单调时钟 (微秒)。
/// 手柄时间戳、帧间隔计算的标准时钟源 (GUI 高频调用)。
pub fn time_now_monotonic_us() -> u64 {
    unsafe { ffi::chiaki_time_now_monotonic_us() }
}

/// C: `chiaki_time_now_monotonic_ms` (static inline, 由 wrap_static_fns 提供)。
pub fn time_now_monotonic_ms() -> u64 {
    unsafe { ffi::chiaki_time_now_monotonic_ms() }
}

/// C: `chiaki_random_32` — 密码学安全的随机 u32。
pub fn random_32() -> u32 {
    unsafe { ffi::chiaki_random_32() }
}

/// C: `chiaki_random_bytes_crypt` — 填充密码学安全随机字节。
pub fn random_bytes_crypt(buf: &mut [u8]) -> Result<(), Error> {
    cvt(unsafe { ffi::chiaki_random_bytes_crypt(buf.as_mut_ptr(), buf.len()) })
}

/// C: `chiaki_base64_encode`。
pub fn base64_encode(input: &[u8]) -> Result<String, Error> {
    // base64 长度上界: 4 * ceil(n/3) + 1
    let mut out = vec![0u8; input.len() / 3 * 4 + 5];
    cvt(unsafe {
        ffi::chiaki_base64_encode(
            input.as_ptr(),
            input.len(),
            out.as_mut_ptr() as *mut std::os::raw::c_char,
            out.len(),
        )
    })?;
    let end = out.iter().position(|&c| c == 0).unwrap_or(out.len());
    Ok(String::from_utf8_lossy(&out[..end]).into_owned())
}

/// C: `chiaki_base64_decode`。
pub fn base64_decode(input: &str) -> Result<Vec<u8>, Error> {
    let c =
        CString::new(input).map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA))?;
    let mut out = vec![0u8; input.len() / 4 * 3 + 3];
    let mut size = out.len();
    cvt(unsafe {
        ffi::chiaki_base64_decode(c.as_ptr(), input.len(), out.as_mut_ptr(), &mut size)
    })?;
    out.truncate(size);
    Ok(out)
}
