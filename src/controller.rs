//! 手柄状态 (`ChiakiControllerState`)。

use crate::ffi;

pub const BUTTON_CROSS: u32 = 1 << 0;
pub const BUTTON_MOON: u32 = 1 << 1;
pub const BUTTON_BOX: u32 = 1 << 2;
pub const BUTTON_PYRAMID: u32 = 1 << 3;
pub const BUTTON_DPAD_LEFT: u32 = 1 << 4;
pub const BUTTON_DPAD_RIGHT: u32 = 1 << 5;
pub const BUTTON_DPAD_UP: u32 = 1 << 6;
pub const BUTTON_DPAD_DOWN: u32 = 1 << 7;
pub const BUTTON_L1: u32 = 1 << 8;
pub const BUTTON_R1: u32 = 1 << 9;
pub const BUTTON_L3: u32 = 1 << 10;
pub const BUTTON_R3: u32 = 1 << 11;
pub const BUTTON_OPTIONS: u32 = 1 << 12;
pub const BUTTON_SHARE: u32 = 1 << 13;
pub const BUTTON_TOUCHPAD: u32 = 1 << 14;
pub const BUTTON_PS: u32 = 1 << 15;
pub const ANALOG_BUTTON_L2: u32 = 1 << 16;
pub const ANALOG_BUTTON_R2: u32 = 1 << 17;

/// 手柄状态。字段直接读写 (`state.0.buttons |= BUTTON_CROSS` 等)。
#[derive(Debug, Clone, Copy)]
pub struct ControllerState(pub ffi::ChiakiControllerState);

impl ControllerState {
    pub fn idle() -> Self {
        let mut s = ffi::ChiakiControllerState::default();
        unsafe { ffi::chiaki_controller_state_set_idle(&mut s) };
        ControllerState(s)
    }

    pub fn equals(&self, other: &Self) -> bool {
        unsafe {
            ffi::chiaki_controller_state_equals(
                &self.0 as *const _ as *mut _,
                &other.0 as *const _ as *mut _,
            )
        }
    }

    /// 合并两个手柄状态 (gyro/accel/orient 取第一个有数据的)。
    pub fn union_with(&self, other: &Self) -> Self {
        let mut out = ffi::ChiakiControllerState::default();
        unsafe {
            ffi::chiaki_controller_state_or(
                &mut out,
                &self.0 as *const _ as *mut _,
                &other.0 as *const _ as *mut _,
            )
        };
        ControllerState(out)
    }

    pub fn start_touch(&mut self, x: u16, y: u16) -> i8 {
        unsafe { ffi::chiaki_controller_state_start_touch(&mut self.0, x, y) }
    }

    pub fn stop_touch(&mut self, id: u8) {
        unsafe { ffi::chiaki_controller_state_stop_touch(&mut self.0, id) }
    }

    pub fn set_touch_pos(&mut self, id: u8, x: u16, y: u16) {
        unsafe { ffi::chiaki_controller_state_set_touch_pos(&mut self.0, id, x, y) }
    }
}
