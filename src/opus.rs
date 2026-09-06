//! 麦克风编解码 (`chiaki/opusencoder.h` / `chiaki/opusdecoder.h`)。
//!
//! 与 C 侧同名 API 一一对应: 编码器把 PCM 帧编码后**自动**经 session 的
//! AudioSender 发送 (`frame()` 内部完成); 解码器产出 [`ffi::ChiakiAudioSink`]
//! (`sink()`) 交给 `Session::set_audio_sink_raw` 接收远端音频并回调 PCM。
//!
//! SAFETY 契约 (与 C 相同的 fini 顺序约束): 编码器/解码器必须比关联的
//! [`Session`] 先 Drop — C 侧也是由调用方保证 fini 顺序。

use std::any::Any;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use crate::error::Error;
use crate::ffi;
use crate::log::Log;
use crate::session::{AudioHeader, Session};
use crate::util::zeroed_box;

/// 麦克风编码器 (`ChiakiOpusEncoder` 的 RAII 封装)。
pub struct OpusEncoder {
    raw: Box<ffi::ChiakiOpusEncoder>,
}

// SAFETY: C 侧自带同步; 所有方法要求 &mut。
unsafe impl Send for OpusEncoder {}

impl OpusEncoder {
    /// C: `chiaki_opus_encoder_init`。
    pub fn new(log: &Log) -> Self {
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiOpusEncoder>() };
        unsafe { ffi::chiaki_opus_encoder_init(&mut *raw, log.as_ptr() as *mut _) };
        OpusEncoder { raw }
    }

    /// C: `chiaki_opus_encoder_header` — 绑定音频参数并关联 session
    /// (内部创建 AudioSender, 之后 `frame()` 自动发送)。
    pub fn header(&mut self, header: &mut AudioHeader, session: &mut Session) {
        unsafe { ffi::chiaki_opus_encoder_header(&mut header.0, &mut *self.raw, session.as_ptr()) };
    }

    /// header 绑定后期望的 PCM 采样数 (frame_size × channels);
    /// 未绑定 header 时为 0。
    pub fn expected_pcm_len(&self) -> usize {
        self.raw.audio_header.frame_size as usize * self.raw.audio_header.channels as usize
    }

    /// C: `chiaki_opus_encoder_frame` — 编码一帧 PCM 并自动发送。
    /// C 侧固定读取 `frame_size × channels` 个采样而不感知缓冲大小,
    /// 这里校验长度: 未绑定 header 或长度不符都会被拒绝 (与 H1 同类
    /// 的安全修复, 见 docs/API_CONSISTENCY_AUDIT.md A1)。
    pub fn frame(&mut self, pcm: &mut [i16]) -> Result<(), Error> {
        let expected = self.expected_pcm_len();
        if expected == 0 || pcm.len() != expected {
            return Err(Error(ffi::ChiakiErrorCode::CHIAKI_ERR_INVALID_DATA));
        }
        unsafe { ffi::chiaki_opus_encoder_frame(pcm.as_mut_ptr(), &mut *self.raw) };
        Ok(())
    }

    /// 音频参数 (header 成功绑定后有效)。
    pub fn audio_header(&self) -> AudioHeader {
        AudioHeader(self.raw.audio_header)
    }
}

impl Drop for OpusEncoder {
    fn drop(&mut self) {
        unsafe { ffi::chiaki_opus_encoder_fini(&mut *self.raw) };
    }
}

/// 解码器两个 C 回调共用的 user 结构 (对应 C 侧单一 cb_user 指针)。
struct DecoderCbs<S, F> {
    settings: Mutex<S>,
    frame: Mutex<F>,
}

unsafe extern "C" fn settings_trampoline<S, F>(channels: u32, rate: u32, user: *mut c_void)
where
    S: FnMut(u32, u32) + Send + 'static,
    F: FnMut(&mut [i16]) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let p = unsafe { &*(user as *const DecoderCbs<S, F>) };
    if let Ok(mut guard) = p.settings.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(channels, rate)));
    }
}

unsafe extern "C" fn frame_trampoline<S, F>(buf: *mut i16, samples_count: usize, user: *mut c_void)
where
    S: FnMut(u32, u32) + Send + 'static,
    F: FnMut(&mut [i16]) + Send + 'static,
{
    if user.is_null() {
        return;
    }
    let p = unsafe { &*(user as *const DecoderCbs<S, F>) };
    let data: &mut [i16] = if buf.is_null() {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(buf, samples_count) }
    };
    if let Ok(mut guard) = p.frame.lock() {
        let _ = catch_unwind(AssertUnwindSafe(|| guard(data)));
    }
}

/// 类型擦除的持有者: 持有堆上的 `DecoderCbs`, `cb_user` 指向它,
/// OpusDecoder Drop 时随 holder 一起释放。
trait CbsHolder: Any + Send {
    fn user_ptr(&self) -> *mut c_void;
}

impl<S, F> CbsHolder for DecoderCbs<S, F>
where
    S: FnMut(u32, u32) + Send + 'static,
    F: FnMut(&mut [i16]) + Send + 'static,
{
    fn user_ptr(&self) -> *mut c_void {
        self as *const _ as *mut c_void
    }
}

/// 麦克风/音频解码器 (`ChiakiOpusDecoder` 的 RAII 封装)。
///
/// 把 [`sink()`](Self::sink) 交给 `Session::set_audio_sink_raw` 后,
/// 远端音频经 session 流入解码器, 解码出的 PCM 回调给 `frame` 闭包。
pub struct OpusDecoder {
    raw: Box<ffi::ChiakiOpusDecoder>,
    _cbs: Option<Box<dyn CbsHolder>>,
}

// SAFETY: C 侧回调只在 session 线程触发, 字段写入后只读;
// 回调经 Mutex 保护且要求 Send。
unsafe impl Send for OpusDecoder {}

impl OpusDecoder {
    /// C: `chiaki_opus_decoder_init`。
    pub fn new(log: &Log) -> Self {
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiOpusDecoder>() };
        unsafe { ffi::chiaki_opus_decoder_init(&mut *raw, log.as_ptr() as *mut _) };
        OpusDecoder { raw, _cbs: None }
    }

    /// C: `chiaki_opus_decoder_set_cb`。
    /// `settings`: (channels, rate), 会话参数变化时触发;
    /// `frame`: 解码出的 PCM (i16 交错采样)。
    pub fn set_cb<S, F>(&mut self, settings: S, frame: F)
    where
        S: FnMut(u32, u32) + Send + 'static,
        F: FnMut(&mut [i16]) + Send + 'static,
    {
        let holder: Box<dyn CbsHolder> = Box::new(DecoderCbs {
            settings: Mutex::new(settings),
            frame: Mutex::new(frame),
        });
        self.raw.settings_cb = Some(settings_trampoline::<S, F>);
        self.raw.frame_cb = Some(frame_trampoline::<S, F>);
        self.raw.cb_user = holder.user_ptr();
        self._cbs = Some(holder);
    }

    /// C: `chiaki_opus_decoder_get_sink` — 产出可交给
    /// `Session::set_audio_sink_raw` 的 sink (类型为 [`ffi::ChiakiAudioSink`])。
    pub fn sink(&mut self) -> ffi::ChiakiAudioSink {
        let mut sink: ffi::ChiakiAudioSink = unsafe { std::mem::zeroed() };
        unsafe { ffi::chiaki_opus_decoder_get_sink(&mut *self.raw, &mut sink) };
        sink
    }

    pub fn audio_header(&self) -> AudioHeader {
        AudioHeader(self.raw.audio_header)
    }
}

impl Drop for OpusDecoder {
    fn drop(&mut self) {
        // 先摘掉 C 侧对回调结构的引用, 再释放 holder。
        self.raw.settings_cb = None;
        self.raw.frame_cb = None;
        self.raw.cb_user = std::ptr::null_mut();
        self._cbs = None;
        unsafe { ffi::chiaki_opus_decoder_fini(&mut *self.raw) };
    }
}
