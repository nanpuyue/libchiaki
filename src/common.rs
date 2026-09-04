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

pub fn target_is_unknown(t: Target) -> bool {
    t == ffi::ChiakiTarget::CHIAKI_TARGET_PS4_UNKNOWN
        || t == ffi::ChiakiTarget::CHIAKI_TARGET_PS5_UNKNOWN
}

pub fn target_is_ps5(t: Target) -> bool {
    t as u32 >= ffi::ChiakiTarget::CHIAKI_TARGET_PS5_UNKNOWN as u32
}

pub fn codec_is_h265(c: Codec) -> bool {
    c == ffi::ChiakiCodec::CHIAKI_CODEC_H265 || c == ffi::ChiakiCodec::CHIAKI_CODEC_H265_HDR
}

pub fn codec_is_hdr(c: Codec) -> bool {
    c == ffi::ChiakiCodec::CHIAKI_CODEC_H265_HDR
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
    r != ffi::ChiakiQuitReason::CHIAKI_QUIT_REASON_STOPPED
        && r != ffi::ChiakiQuitReason::CHIAKI_QUIT_REASON_STREAM_CONNECTION_REMOTE_SHUTDOWN
}
