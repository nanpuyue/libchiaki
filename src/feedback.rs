//! 反馈历史环形缓冲 (`ChiakiFeedbackHistoryBuffer`)。

use crate::error::{Error, cvt};
use crate::ffi;
use crate::util::zeroed_box;

/// `ChiakiFeedbackHistoryEvent` builder。
#[derive(Debug, Clone, Copy)]
pub struct HistoryEvent(pub ffi::ChiakiFeedbackHistoryEvent);

impl HistoryEvent {
    pub fn new() -> Self {
        HistoryEvent(unsafe { std::mem::zeroed() })
    }

    pub fn set_button(&mut self, button: u64, state: u8) -> Result<(), Error> {
        cvt(unsafe { ffi::chiaki_feedback_history_event_set_button(&mut self.0, button, state) })
    }

    pub fn set_touchpad(&mut self, down: bool, pointer_id: u8, x: u16, y: u16) {
        unsafe {
            ffi::chiaki_feedback_history_event_set_touchpad(&mut self.0, down, pointer_id, x, y)
        };
    }
}

impl Default for HistoryEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// `ChiakiFeedbackHistoryBuffer` 封装。
pub struct HistoryBuffer {
    raw: Box<ffi::ChiakiFeedbackHistoryBuffer>,
}

// SAFETY: 纯数据结构, 方法全是 &mut, 无内部共享状态。
unsafe impl Send for HistoryBuffer {}

impl HistoryBuffer {
    pub fn new(size: usize) -> Result<Self, Error> {
        let mut raw = unsafe { zeroed_box::<ffi::ChiakiFeedbackHistoryBuffer>() };
        cvt(unsafe { ffi::chiaki_feedback_history_buffer_init(&mut *raw, size) })?;
        Ok(HistoryBuffer { raw })
    }

    pub fn push(&mut self, event: &HistoryEvent) {
        unsafe {
            ffi::chiaki_feedback_history_buffer_push(
                &mut *self.raw,
                &event.0 as *const _ as *mut _,
            )
        };
    }

    /// 序列化 (自动扩容, 上限 1MB)。
    pub fn format(&mut self) -> Result<Vec<u8>, Error> {
        let mut size = 256usize;
        loop {
            let mut buf = vec![0u8; size];
            let mut out_size = size;
            let rc = unsafe {
                ffi::chiaki_feedback_history_buffer_format(
                    &mut *self.raw,
                    buf.as_mut_ptr(),
                    &mut out_size,
                )
            };
            if rc == ffi::ChiakiErrorCode::CHIAKI_ERR_BUF_TOO_SMALL {
                size *= 2;
                if size > 1024 * 1024 {
                    return Err(Error(rc));
                }
                continue;
            }
            cvt(rc)?;
            buf.truncate(out_size);
            return Ok(buf);
        }
    }
}

impl Drop for HistoryBuffer {
    fn drop(&mut self) {
        unsafe { ffi::chiaki_feedback_history_buffer_fini(&mut *self.raw) };
    }
}
