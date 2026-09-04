//! C 垫片的 Rust 声明 (实现见 shim/chiaki_shim.c, 由 build.rs 编译链接)。
//!
//! 提供两类函数: 各结构体的真实 sizeof / alignof,
//! 以及 chiaki 头文件 `static inline` 辅助函数的薄封装
//! (后者不在静态库里, 不能直接链接)。

use std::os::raw::c_void;

use crate::ffi;

#[allow(dead_code)] // 多数仅布局测试用。
unsafe extern "C" {
    pub fn libchiaki_sizeof_ChiakiSession() -> usize;
    pub fn libchiaki_alignof_ChiakiSession() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscoveryService() -> usize;
    pub fn libchiaki_alignof_ChiakiDiscoveryService() -> usize;
    pub fn libchiaki_sizeof_ChiakiLog() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscovery() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscoveryThread() -> usize;
    pub fn libchiaki_sizeof_ChiakiRegist() -> usize;
    pub fn libchiaki_sizeof_ChiakiConnectInfo() -> usize;
    pub fn libchiaki_sizeof_ChiakiRegistInfo() -> usize;
    pub fn libchiaki_sizeof_ChiakiControllerState() -> usize;
    pub fn libchiaki_sizeof_ChiakiControllerTouch() -> usize;
    pub fn libchiaki_sizeof_ChiakiEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiQuitEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiKeyboardEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiRumbleEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiTriggerEffectsEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiVideoFecFailureEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscoveryHost() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscoveryPacket() -> usize;
    pub fn libchiaki_sizeof_ChiakiRegisteredHost() -> usize;
    pub fn libchiaki_sizeof_ChiakiRegistEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiAudioHeader() -> usize;
    pub fn libchiaki_sizeof_ChiakiAudioSink() -> usize;
    pub fn libchiaki_sizeof_ChiakiCtrlDisplaySink() -> usize;
    pub fn libchiaki_sizeof_ChiakiDiscoveryServiceOptions() -> usize;
    pub fn libchiaki_sizeof_ChiakiFeedbackHistoryBuffer() -> usize;
    pub fn libchiaki_sizeof_ChiakiFeedbackHistoryEvent() -> usize;
    pub fn libchiaki_sizeof_ChiakiHolepunchRegistInfo() -> usize;
    pub fn libchiaki_sizeof_ChiakiHolepunchDeviceInfo() -> usize;
    pub fn libchiaki_sizeof_ChiakiConnectVideoProfile() -> usize;
    pub fn libchiaki_session_set_event_cb(
        s: *mut ffi::ChiakiSession,
        cb: ffi::ChiakiEventCallback,
        user: *mut c_void,
    );
    pub fn libchiaki_session_set_video_sample_cb(
        s: *mut ffi::ChiakiSession,
        cb: ffi::ChiakiVideoSampleCallback,
        user: *mut c_void,
    );
    pub fn libchiaki_session_set_audio_sink(
        s: *mut ffi::ChiakiSession,
        sink: *const ffi::ChiakiAudioSink,
    );
    pub fn libchiaki_session_set_haptics_sink(
        s: *mut ffi::ChiakiSession,
        sink: *const ffi::ChiakiAudioSink,
    );
    pub fn libchiaki_session_set_display_sink(
        s: *mut ffi::ChiakiSession,
        sink: *const ffi::ChiakiCtrlDisplaySink,
    );
}
