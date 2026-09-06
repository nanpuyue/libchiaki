//! 姿态/体感 (`chiaki/orientation.h`): 陀螺仪+加速度计的姿态解算。
//!
//! 典型流程 (与 C 侧一致): 持有一个 [`AccelNewZero`] 记录"重力归零"基准,
//! 手柄上报原始陀螺仪/加速度数据时调 [`OrientationTracker::update`],
//! 再 [`OrientationTracker::apply_to_controller_state`] 把姿态写进手柄状态。

use crate::controller::ControllerState;
use crate::ffi;

/// 加速度计零点基准 (`ChiakiAccelNewZero`)。
#[derive(Debug, Clone, Copy)]
pub struct AccelNewZero(pub ffi::ChiakiAccelNewZero);

impl AccelNewZero {
    pub fn new() -> Self {
        let mut z: ffi::ChiakiAccelNewZero = unsafe { std::mem::zeroed() };
        unsafe { ffi::chiaki_accel_new_zero_set_inactive(&mut z, false) };
        AccelNewZero(z)
    }

    /// C: `chiaki_accel_new_zero_set_active` — 以当前读数为新的重力基准。
    pub fn set_active(&mut self, accel_x: f32, accel_y: f32, accel_z: f32, real_accel: bool) {
        unsafe {
            ffi::chiaki_accel_new_zero_set_active(
                &mut self.0,
                accel_x,
                accel_y,
                accel_z,
                real_accel,
            )
        };
    }

    /// C: `chiaki_accel_new_zero_set_inactive`。
    pub fn set_inactive(&mut self, real_accel: bool) {
        unsafe { ffi::chiaki_accel_new_zero_set_inactive(&mut self.0, real_accel) };
    }
}

impl Default for AccelNewZero {
    fn default() -> Self {
        Self::new()
    }
}

/// 单次姿态解算结果 (`ChiakiOrientation`)。
#[derive(Debug, Clone, Copy)]
pub struct Orientation(pub ffi::ChiakiOrientation);

impl Orientation {
    pub fn new() -> Self {
        let mut o: ffi::ChiakiOrientation = unsafe { std::mem::zeroed() };
        unsafe { ffi::chiaki_orientation_init(&mut o) };
        Orientation(o)
    }

    /// 陀螺仪 (gx, gy, gz) + 加速度 (ax, ay, az) + 磁力计 (beta) 解算一步。
    // 参数个数与 C 原型一一对应, 忠实签名优先 (clippy::too_many_arguments)。
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        gx: f32,
        gy: f32,
        gz: f32,
        ax: f32,
        ay: f32,
        az: f32,
        beta: f32,
        time_step_sec: f32,
    ) {
        unsafe {
            ffi::chiaki_orientation_update(&mut self.0, gx, gy, gz, ax, ay, az, beta, time_step_sec)
        };
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Self::new()
    }
}

/// 姿态跟踪器 (`ChiakiOrientationTracker`), 内部维护历史以做漂移补偿。
#[derive(Debug, Clone, Copy)]
pub struct OrientationTracker(pub ffi::ChiakiOrientationTracker);

impl OrientationTracker {
    pub fn new() -> Self {
        let mut t: ffi::ChiakiOrientationTracker = unsafe { std::mem::zeroed() };
        unsafe { ffi::chiaki_orientation_tracker_init(&mut t) };
        OrientationTracker(t)
    }

    /// C: `chiaki_orientation_tracker_update`。`timestamp_us` 用于两帧间隔。
    // 参数个数与 C 原型一一对应, 忠实签名优先 (clippy::too_many_arguments)。
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        gx: f32,
        gy: f32,
        gz: f32,
        ax: f32,
        ay: f32,
        az: f32,
        accel_zero: &mut AccelNewZero,
        accel_zero_applied: bool,
        timestamp_us: u32,
    ) {
        unsafe {
            ffi::chiaki_orientation_tracker_update(
                &mut self.0,
                gx,
                gy,
                gz,
                ax,
                ay,
                az,
                &mut accel_zero.0,
                accel_zero_applied,
                timestamp_us,
            )
        };
    }

    /// C: `chiaki_orientation_tracker_apply_to_controller_state` —
    /// 把姿态写进手柄状态的 gyro/orient 字段。
    pub fn apply_to_controller_state(&mut self, state: &mut ControllerState) {
        unsafe {
            ffi::chiaki_orientation_tracker_apply_to_controller_state(&mut self.0, &mut state.0)
        };
    }
}

impl Default for OrientationTracker {
    fn default() -> Self {
        Self::new()
    }
}
