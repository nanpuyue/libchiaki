//! 连接信息 (`ChiakiConnectInfo` 的 builder)。

use std::ffi::{CString, NulError};
use std::os::raw::c_char;

use crate::common::Codec;
use crate::ffi;
use crate::util::zeroed_box;

pub type ResolutionPreset = ffi::ChiakiVideoResolutionPreset;
pub type FpsPreset = ffi::ChiakiVideoFPSPreset;

/// 按分辨率 / 帧率预设生成 video profile。
pub fn video_profile_preset(
    res: ResolutionPreset,
    fps: FpsPreset,
) -> ffi::ChiakiConnectVideoProfile {
    let mut p = ffi::ChiakiConnectVideoProfile::default();
    unsafe { ffi::chiaki_connect_video_profile_preset(&mut p, res, fps) };
    p
}

/// `ChiakiConnectInfo` 的 builder。拥有 host 字符串,
/// Session 会接管本对象, 生命周期安全。
pub struct ConnectInfo {
    raw: Box<ffi::ChiakiConnectInfo>,
    _host: CString,
}

// SAFETY: 内部裸指针全部指向自身拥有的 CString / 数组,
// 单一所有权, 跨线程转移安全 (C 侧 init 时只读取)。
unsafe impl Send for ConnectInfo {}

impl ConnectInfo {
    /// 默认 720p30, 其余字段按需用 setter 改。
    pub fn new(host: &str, regist_key: &[u8; 16], ps5: bool) -> Result<Self, NulError> {
        let host_c = CString::new(host)?;
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiConnectInfo>() };
        raw.ps5 = ps5;
        raw.host = host_c.as_ptr();
        for (i, b) in regist_key.iter().enumerate() {
            raw.regist_key[i] = *b as c_char;
        }
        raw.video_profile = video_profile_preset(
            ffi::ChiakiVideoResolutionPreset::CHIAKI_VIDEO_RESOLUTION_PRESET_720p,
            ffi::ChiakiVideoFPSPreset::CHIAKI_VIDEO_FPS_PRESET_30,
        );
        Ok(ConnectInfo { raw, _host: host_c })
    }

    pub fn set_video_preset(&mut self, res: ResolutionPreset, fps: FpsPreset) {
        self.raw.video_profile = video_profile_preset(res, fps);
    }

    pub fn set_video_codec(&mut self, codec: Codec) {
        self.raw.video_profile.codec = codec;
    }

    /// 设置比特率 (kbps)。可在 preset 之后调用, 覆盖预设默认值;
    pub fn set_bitrate(&mut self, bitrate: u32) {
        self.raw.video_profile.bitrate = bitrate;
    }

    /// 获取当前 video profile 的比特率 (kbps)。
    pub fn bitrate(&self) -> u32 {
        self.raw.video_profile.bitrate
    }

    pub fn set_video_custom(
        &mut self,
        width: u32,
        height: u32,
        max_fps: u32,
        bitrate: u32,
        codec: Codec,
    ) {
        self.raw.video_profile = ffi::ChiakiConnectVideoProfile {
            width,
            height,
            max_fps,
            bitrate,
            codec,
        };
    }

    pub fn set_psn_account_id(&mut self, id: &[u8; 8]) {
        self.raw.psn_account_id = *id;
    }

    pub fn set_morning(&mut self, morning: &[u8; 16]) {
        self.raw.morning = *morning;
    }

    pub fn set_auto_downgrade(&mut self, v: bool) {
        self.raw.video_profile_auto_downgrade = v;
    }
    pub fn set_enable_keyboard(&mut self, v: bool) {
        self.raw.enable_keyboard = v;
    }
    pub fn set_enable_dualsense(&mut self, v: bool) {
        self.raw.enable_dualsense = v;
    }
    pub fn set_auto_regist(&mut self, v: bool) {
        self.raw.auto_regist = v;
    }
    pub fn set_packet_loss_max(&mut self, v: f64) {
        self.raw.packet_loss_max = v;
    }
    pub fn set_enable_idr_on_fec_failure(&mut self, v: bool) {
        self.raw.enable_idr_on_fec_failure = v;
    }
    pub fn set_audio_video_disabled(&mut self, v: ffi::ChiakiDisableAudioVideo) {
        self.raw.audio_video_disabled = v;
    }

    pub(crate) fn as_ptr(&self) -> *const ffi::ChiakiConnectInfo {
        &*self.raw
    }
}
