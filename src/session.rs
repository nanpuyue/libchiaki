//! 流 Session: 事件、视频/音频回调、手柄输入、生命周期管理。

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::CString;
use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;
use std::sync::Mutex;

use crate::common::{DualSenseIntensity, QuitReason};
use crate::connect::ConnectInfo;
use crate::controller::ControllerState;
use crate::error::{Error, cvt};
use crate::ffi;
use crate::log::Log;
use crate::util::{ErasedCallback, cchar_array_to_string, cstr_to_string};

/// 注册成功时返回的主机信息 (拥有所有权)。
#[derive(Debug, Clone)]
pub struct RegisteredHost {
    pub target: crate::common::Target,
    pub ap_ssid: String,
    pub ap_bssid: String,
    pub ap_key: String,
    pub ap_name: String,
    pub server_mac: [u8; 6],
    pub server_nickname: String,
    pub rp_regist_key: [u8; 16],
    pub rp_key_type: u32,
    pub rp_key: [u8; 16],
    pub console_pin: u32,
}

pub(crate) fn parse_registered_host(h: &ffi::ChiakiRegisteredHost) -> RegisteredHost {
    let rp_regist_key = h.rp_regist_key;
    RegisteredHost {
        target: h.target,
        ap_ssid: cchar_array_to_string(&h.ap_ssid),
        ap_bssid: cchar_array_to_string(&h.ap_bssid),
        ap_key: cchar_array_to_string(&h.ap_key),
        ap_name: cchar_array_to_string(&h.ap_name),
        server_mac: h.server_mac,
        server_nickname: cchar_array_to_string(&h.server_nickname),
        rp_regist_key: rp_regist_key.map(|c| c as u8),
        rp_key_type: h.rp_key_type,
        rp_key: h.rp_key,
        console_pin: h.console_pin,
    }
}

/// Session 事件 (拥有所有权, 可跨线程传递)。
#[derive(Debug, Clone)]
pub enum Event {
    Connected,
    LoginPinRequest {
        pin_incorrect: bool,
    },
    Holepunch {
        finished: bool,
    },
    Regist(RegisteredHost),
    NicknameReceived(String),
    KeyboardOpen,
    KeyboardTextChange(String),
    KeyboardRemoteClose,
    Rumble {
        unknown: u8,
        left: u8,
        right: u8,
    },
    Quit {
        reason: QuitReason,
        reason_str: String,
    },
    TriggerEffects {
        type_left: u8,
        type_right: u8,
        left: [u8; 10],
        right: [u8; 10],
    },
    MotionReset,
    LedColor([u8; 3]),
    PlayerIndex(u8),
    HapticIntensity(DualSenseIntensity),
    TriggerIntensity(DualSenseIntensity),
    VideoFecFailure {
        frame_index: i32,
        idr_request_sent: bool,
    },
}

fn parse_event(ev: &ffi::ChiakiEvent) -> Event {
    use ffi::ChiakiEventType::*;
    unsafe {
        let u = &ev.__bindgen_anon_1;
        match ev.type_ {
            CHIAKI_EVENT_CONNECTED => Event::Connected,
            CHIAKI_EVENT_LOGIN_PIN_REQUEST => Event::LoginPinRequest {
                pin_incorrect: u.login_pin_request.pin_incorrect,
            },
            CHIAKI_EVENT_HOLEPUNCH => Event::Holepunch {
                finished: u.data_holepunch.finished,
            },
            CHIAKI_EVENT_REGIST => Event::Regist(parse_registered_host(&u.host)),
            CHIAKI_EVENT_NICKNAME_RECEIVED => {
                Event::NicknameReceived(cchar_array_to_string(&u.server_nickname))
            }
            CHIAKI_EVENT_KEYBOARD_OPEN => Event::KeyboardOpen,
            CHIAKI_EVENT_KEYBOARD_TEXT_CHANGE => {
                Event::KeyboardTextChange(cstr_to_string(u.keyboard.text_str))
            }
            CHIAKI_EVENT_KEYBOARD_REMOTE_CLOSE => Event::KeyboardRemoteClose,
            CHIAKI_EVENT_RUMBLE => Event::Rumble {
                unknown: u.rumble.unknown,
                left: u.rumble.left,
                right: u.rumble.right,
            },
            CHIAKI_EVENT_QUIT => Event::Quit {
                reason: u.quit.reason,
                reason_str: cstr_to_string(u.quit.reason_str),
            },
            CHIAKI_EVENT_TRIGGER_EFFECTS => Event::TriggerEffects {
                type_left: u.trigger_effects.type_left,
                type_right: u.trigger_effects.type_right,
                left: u.trigger_effects.left,
                right: u.trigger_effects.right,
            },
            CHIAKI_EVENT_MOTION_RESET => Event::MotionReset,
            CHIAKI_EVENT_LED_COLOR => Event::LedColor(u.led_state),
            CHIAKI_EVENT_PLAYER_INDEX => Event::PlayerIndex(u.player_index),
            CHIAKI_EVENT_HAPTIC_INTENSITY => Event::HapticIntensity(u.intensity),
            CHIAKI_EVENT_TRIGGER_INTENSITY => Event::TriggerIntensity(u.intensity),
            CHIAKI_EVENT_VIDEO_FEC_FAILURE => Event::VideoFecFailure {
                frame_index: u.video_fec_failure.frame_index,
                idr_request_sent: u.video_fec_failure.idr_request_sent,
            },
        }
    }
}

/// 视频帧回调参数。`data` 仅回调期间有效 (含 FFmpeg 要求的 padding)。
pub struct VideoSample<'a> {
    pub data: &'a mut [u8],
    pub frames_lost: i32,
    pub frame_recovered: bool,
}

unsafe extern "C" fn event_trampoline<F>(event: *mut ffi::ChiakiEvent, user: *mut c_void)
where
    F: FnMut(Event) + Send + 'static,
{
    if event.is_null() || user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let ev = parse_event(unsafe { &*event });
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(ev)));
    }
}

unsafe extern "C" fn video_trampoline<F>(
    buf: *mut u8,
    buf_size: usize,
    frames_lost: i32,
    frame_recovered: bool,
    user: *mut c_void,
) -> bool
where
    F: FnMut(VideoSample) -> bool + Send + 'static,
{
    if user.is_null() {
        return false;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    let data: &mut [u8] = if buf.is_null() {
        &mut []
    } else {
        unsafe { slice::from_raw_parts_mut(buf, buf_size) }
    };
    if let Ok(mut guard) = m.lock() {
        let sample = VideoSample {
            data,
            frames_lost,
            frame_recovered,
        };
        return catch_unwind(AssertUnwindSafe(|| guard(sample))).unwrap_or(false);
    }
    false
}

/// 音频 sink 的两个闭包放一个 Mutex 里 (共用一个 user 指针)。
struct AudioSinkPair<H, F> {
    header_cb: H,
    frame_cb: F,
}

unsafe extern "C" fn audio_sink_header_trampoline<H, F>(
    header: *mut ffi::ChiakiAudioHeader,
    user: *mut c_void,
) where
    H: FnMut(&mut AudioHeader) + Send + 'static,
    F: FnMut(&mut [u8]) + Send + 'static,
{
    if header.is_null() || user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<AudioSinkPair<H, F>>) };
    if let Ok(mut guard) = m.lock() {
        let h = unsafe { &mut *(header as *mut AudioHeader) };
        let _ = catch_unwind(AssertUnwindSafe(|| (guard.header_cb)(h)));
    }
}

unsafe extern "C" fn audio_sink_frame_trampoline<H, F>(
    buf: *mut u8,
    buf_size: usize,
    user: *mut c_void,
) where
    H: FnMut(&mut AudioHeader) + Send + 'static,
    F: FnMut(&mut [u8]) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<AudioSinkPair<H, F>>) };
    let data: &mut [u8] = if buf.is_null() {
        &mut []
    } else {
        unsafe { slice::from_raw_parts_mut(buf, buf_size) }
    };
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| (guard.frame_cb)(data)));
    }
}

unsafe extern "C" fn display_sink_trampoline<F>(user: *mut c_void, cant_display: bool)
where
    F: FnMut(bool) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let m = unsafe { &*(user as *const Mutex<F>) };
    if let Ok(mut guard) = m.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(cant_display)));
    }
}

/// 流 Session。内存由 C 垫片按真实大小分配, Rust 只持有指针。
///
/// 回调必须在 `start()` 之前设置; `Drop` 会依次 stop / join / fini
/// (可能阻塞, 想精确控制请手动先调 `stop()` / `join()`)。
pub struct Session<'a> {
    ptr: *mut ffi::ChiakiSession,
    layout: Layout,
    _info: ConnectInfo,
    _event_cb: Option<ErasedCallback>,
    _video_cb: Option<ErasedCallback>,
    _audio_sink: Option<ErasedCallback>,
    _haptics_sink: Option<ErasedCallback>,
    _display_sink: Option<ErasedCallback>,
    joined: bool,
    // 生命周期只用于追踪 log 的借用; Log / LogSniffer 都 Deref 到
    // ffi::ChiakiLog, 见 new / new_with_raw_log。
    _log: PhantomData<&'a ffi::ChiakiLog>,
}

// SAFETY: chiaki 的 session API 为跨线程使用设计
// (手柄状态就是从别的线程设置的); 所有方法都要求 &mut,
// 不同时存在共享引用, 回调经 Mutex 保护且要求 Send。
unsafe impl Send for Session<'_> {}

impl<'a> Session<'a> {
    pub fn new(info: ConnectInfo, log: &'a Log) -> Result<Self, Error> {
        Self::new_with_raw_log(info, log)
    }

    /// 同 [`new`](Self::new), 但直接接受 C 侧的 `ChiakiLog *` —
    /// 允许把 [`LogSniffer`](crate::log::LogSniffer) 的 sniff log
    /// 交给 session (Deref 强制转换会把 `&LogSniffer` 变成 `&ChiakiLog`)。
    pub fn new_with_raw_log(info: ConnectInfo, log: &'a ffi::ChiakiLog) -> Result<Self, Error> {
        let size = size_of::<ffi::ChiakiSession>();
        let align = align_of::<ffi::ChiakiSession>();
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_UNKNOWN))?;
        let ptr = unsafe { alloc(layout) as *mut ffi::ChiakiSession };
        if ptr.is_null() {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_MEMORY));
        }
        unsafe { ptr::write_bytes(ptr as *mut u8, 0, size) };
        let rc = unsafe {
            ffi::chiaki_session_init(ptr, info.as_ptr() as *mut _, log as *const _ as *mut _)
        };
        if let Err(e) = cvt(rc) {
            unsafe { dealloc(ptr as *mut u8, layout) };
            return Err(e);
        }
        Ok(Session {
            ptr,
            layout,
            _info: info,
            _event_cb: None,
            _video_cb: None,
            _audio_sink: None,
            _haptics_sink: None,
            _display_sink: None,
            joined: false,
            _log: PhantomData,
        })
    }

    pub fn start(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_start(self.ptr) })
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_stop(self.ptr) })
    }

    pub fn join(&mut self) -> Result<(), Error> {
        let r = cvt(unsafe { ffi::chiaki_session_join(self.ptr) });
        if r.is_ok() {
            self.joined = true;
        }
        r
    }

    pub fn set_controller_state(&mut self, state: &ControllerState) -> Result<(), Error> {
        cvt(unsafe {
            ffi::chiaki_session_set_controller_state(self.ptr, &state.0 as *const _ as *mut _)
        })
    }

    pub fn set_login_pin(&mut self, pin: &[u8]) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_set_login_pin(self.ptr, pin.as_ptr(), pin.len()) })
    }

    pub fn request_idr(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_request_idr(self.ptr) })
    }

    pub fn goto_bed(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_goto_bed(self.ptr) })
    }

    pub fn go_home(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_go_home(self.ptr) })
    }

    pub fn toggle_microphone(&mut self, muted: bool) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_toggle_microphone(self.ptr, muted) })
    }

    pub fn connect_microphone(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_connect_microphone(self.ptr) })
    }

    pub fn keyboard_set_text(&mut self, text: &str) -> Result<(), Error> {
        let c =
            CString::new(text).map_err(|_| Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA))?;
        cvt(unsafe { ffi::chiaki_session_keyboard_set_text(self.ptr, c.as_ptr()) })
    }

    pub fn keyboard_accept(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_keyboard_accept(self.ptr) })
    }

    pub fn keyboard_reject(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_keyboard_reject(self.ptr) })
    }

    pub fn set_stream_connection_switch_received(&mut self) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_session_set_stream_connection_switch_received(self.ptr) })
    }

    /// 设置事件回调 (start 之前调用; 回调在 chiaki 内部线程触发)。
    pub fn set_event_callback<F>(&mut self, f: F)
    where
        F: FnMut(Event) + Send + 'static,
    {
        let holder = ErasedCallback::new(f);
        unsafe {
            ffi::chiaki_session_set_event_cb(self.ptr, Some(event_trampoline::<F>), holder.ptr)
        };
        self._event_cb = Some(holder);
    }

    /// 设置视频帧回调。返回 false 表示解码失败, chiaki 会请求关键帧。
    pub fn set_video_sample_callback<F>(&mut self, f: F)
    where
        F: FnMut(VideoSample) -> bool + Send + 'static,
    {
        let holder = ErasedCallback::new(f);
        unsafe {
            ffi::chiaki_session_set_video_sample_cb(
                self.ptr,
                Some(video_trampoline::<F>),
                holder.ptr,
            )
        };
        self._video_cb = Some(holder);
    }

    /// 写入 session->audio_sink (C 侧拷贝结构体)。
    pub fn set_audio_sink<H, F>(&mut self, header_cb: H, frame_cb: F)
    where
        H: FnMut(&mut AudioHeader) + Send + 'static,
        F: FnMut(&mut [u8]) + Send + 'static,
    {
        let holder = ErasedCallback::new(AudioSinkPair {
            header_cb,
            frame_cb,
        });
        let mut sink = ffi::ChiakiAudioSink {
            user: holder.ptr,
            header_cb: Some(audio_sink_header_trampoline::<H, F>),
            frame_cb: Some(audio_sink_frame_trampoline::<H, F>),
        };
        unsafe { ffi::chiaki_session_set_audio_sink(self.ptr, &mut sink) };
        self._audio_sink = Some(holder);
    }

    pub fn set_haptics_sink<H, F>(&mut self, header_cb: H, frame_cb: F)
    where
        H: FnMut(&mut AudioHeader) + Send + 'static,
        F: FnMut(&mut [u8]) + Send + 'static,
    {
        let holder = ErasedCallback::new(AudioSinkPair {
            header_cb,
            frame_cb,
        });
        let mut sink = ffi::ChiakiAudioSink {
            user: holder.ptr,
            header_cb: Some(audio_sink_header_trampoline::<H, F>),
            frame_cb: Some(audio_sink_frame_trampoline::<H, F>),
        };
        unsafe { ffi::chiaki_session_set_haptics_sink(self.ptr, &mut sink) };
        self._haptics_sink = Some(holder);
    }

    pub fn set_display_sink<F>(&mut self, f: F)
    where
        F: FnMut(bool) + Send + 'static,
    {
        let holder = ErasedCallback::new(f);
        let mut sink = ffi::ChiakiCtrlDisplaySink {
            user: holder.ptr,
            cantdisplay_cb: Some(display_sink_trampoline::<F>),
        };
        unsafe { ffi::chiaki_session_ctrl_set_display_sink(self.ptr, &mut sink) };
        self._display_sink = Some(holder);
    }

    /// 内部 `ChiakiSession *` (供关联封装如 OpusEncoder 使用)。
    pub fn as_ptr(&self) -> *mut ffi::ChiakiSession {
        self.ptr
    }

    /// C: `chiaki_video_receiver_set_waiting_for_idr` — 手动请求
    /// 等待 IDR 帧 (丢包恢复后强制刷新画面时用)。
    pub fn video_receiver_set_waiting_for_idr(&mut self, waiting: bool) {
        let vr = unsafe { (*self.ptr).stream_connection.video_receiver };
        if !vr.is_null() {
            unsafe { ffi::chiaki_video_receiver_set_waiting_for_idr(vr, waiting) };
        }
    }

    /// C: `chiaki_video_receiver_get_waiting_for_idr`。
    pub fn video_receiver_waiting_for_idr(&self) -> bool {
        let vr = unsafe { (*self.ptr).stream_connection.video_receiver };
        !vr.is_null() && unsafe { ffi::chiaki_video_receiver_get_waiting_for_idr(vr) }
    }

    /// C: `chiaki_video_receiver_get_frames_lost_total`。
    pub fn video_receiver_frames_lost_total(&self) -> i32 {
        let vr = unsafe { (*self.ptr).stream_connection.video_receiver };
        if vr.is_null() {
            0
        } else {
            unsafe { ffi::chiaki_video_receiver_get_frames_lost_total(vr) }
        }
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        unsafe {
            // 最佳努力: 先停、等线程结束 (回调 holder 在字段 drop 时才释放,
            // 此时 C 线程已结束, 无 UAF)。
            let _ = ffi::chiaki_session_stop(self.ptr);
            if !self.joined {
                let _ = ffi::chiaki_session_join(self.ptr);
            }
            ffi::chiaki_session_fini(self.ptr);
            dealloc(self.ptr as *mut u8, self.layout);
        }
    }
}

/// `ChiakiAudioHeader` 薄封装。
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct AudioHeader(pub ffi::ChiakiAudioHeader);

impl AudioHeader {
    pub fn new(channels: u8, bits: u8, rate: u32, frame_size: u32) -> Self {
        let mut h = ffi::ChiakiAudioHeader::default();
        unsafe { ffi::chiaki_audio_header_set(&mut h, channels, bits, rate, frame_size) };
        AudioHeader(h)
    }

    /// C: `chiaki_audio_header_load` — 从序列化字节 (14 字节) 反序列化。
    pub fn load(buf: &[u8]) -> Self {
        let mut h: ffi::ChiakiAudioHeader = unsafe { std::mem::zeroed() };
        unsafe { ffi::chiaki_audio_header_load(&mut h, buf.as_ptr()) };
        AudioHeader(h)
    }

    /// C: `chiaki_audio_header_save` — 序列化到 `buf` (需 ≥ 14 字节)。
    pub fn save(&mut self, buf: &mut [u8]) {
        unsafe { ffi::chiaki_audio_header_save(&mut self.0 as *mut _, buf.as_mut_ptr()) };
    }

    pub fn frame_bytes(&self) -> usize {
        self.0.frame_size as usize * self.0.channels as usize * 2
    }
}
