//! 手柄状态 (`ChiakiControllerState`)。
//!
//! 按钮位常量直接取自 ffi 的 `ChiakiControllerButton` /
//! `ChiakiControllerAnalogButton` 枚举, 不手写位值。

use crate::ffi;

pub use ffi::ChiakiControllerAnalogButton as AnalogButton;
pub use ffi::ChiakiControllerButton as Button;

/// 按钮位掩码常量 (值来自 ffi 枚举, 与 C 的 `1 << n` 一致)。
pub const BUTTON_CROSS: u32 = Button::CHIAKI_CONTROLLER_BUTTON_CROSS as u32;
pub const BUTTON_MOON: u32 = Button::CHIAKI_CONTROLLER_BUTTON_MOON as u32;
pub const BUTTON_BOX: u32 = Button::CHIAKI_CONTROLLER_BUTTON_BOX as u32;
pub const BUTTON_PYRAMID: u32 = Button::CHIAKI_CONTROLLER_BUTTON_PYRAMID as u32;
pub const BUTTON_DPAD_LEFT: u32 = Button::CHIAKI_CONTROLLER_BUTTON_DPAD_LEFT as u32;
pub const BUTTON_DPAD_RIGHT: u32 = Button::CHIAKI_CONTROLLER_BUTTON_DPAD_RIGHT as u32;
pub const BUTTON_DPAD_UP: u32 = Button::CHIAKI_CONTROLLER_BUTTON_DPAD_UP as u32;
pub const BUTTON_DPAD_DOWN: u32 = Button::CHIAKI_CONTROLLER_BUTTON_DPAD_DOWN as u32;
pub const BUTTON_L1: u32 = Button::CHIAKI_CONTROLLER_BUTTON_L1 as u32;
pub const BUTTON_R1: u32 = Button::CHIAKI_CONTROLLER_BUTTON_R1 as u32;
pub const BUTTON_L3: u32 = Button::CHIAKI_CONTROLLER_BUTTON_L3 as u32;
pub const BUTTON_R3: u32 = Button::CHIAKI_CONTROLLER_BUTTON_R3 as u32;
pub const BUTTON_OPTIONS: u32 = Button::CHIAKI_CONTROLLER_BUTTON_OPTIONS as u32;
pub const BUTTON_SHARE: u32 = Button::CHIAKI_CONTROLLER_BUTTON_SHARE as u32;
pub const BUTTON_TOUCHPAD: u32 = Button::CHIAKI_CONTROLLER_BUTTON_TOUCHPAD as u32;
pub const BUTTON_PS: u32 = Button::CHIAKI_CONTROLLER_BUTTON_PS as u32;
pub const ANALOG_BUTTON_L2: u32 = AnalogButton::CHIAKI_CONTROLLER_ANALOG_BUTTON_L2 as u32;
pub const ANALOG_BUTTON_R2: u32 = AnalogButton::CHIAKI_CONTROLLER_ANALOG_BUTTON_R2 as u32;

/// 手柄状态。字段直接读写 (`state.0.buttons |= BUTTON_CROSS` 等)。
#[derive(Debug, Clone, Copy)]
pub struct ControllerState(pub ffi::ChiakiControllerState);

impl ControllerState {
    pub fn idle() -> Self {
        let mut s = ffi::ChiakiControllerState::default();
        unsafe { ffi::chiaki_controller_state_set_idle(&mut s) };
        ControllerState(s)
    }

    /// C: `chiaki_controller_state_set_idle` — 原地清零成空闲态
    /// (每帧汇总各输入源状态前调用, GUI 的最高频操作之一)。
    pub fn set_idle(&mut self) {
        unsafe { ffi::chiaki_controller_state_set_idle(&mut self.0) };
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
