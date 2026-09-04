//! 运行时布局校验: bindgen 布局 vs C 真实布局。
//!
//! 任何一条失败都说明封装用错了内存大小, 必须修。
//! (bindgen 对部分前向声明的结构体生成 opaque 占位,
//! 见 build.rs 注释, 因此这里逐个核对。)

use std::mem::size_of;

use crate::{ffi, shim};

#[test]
fn layouts_match_c() {
    unsafe fn check<T>(rust_size: usize, c_size: usize, name: &str) {
        assert_eq!(rust_size, c_size, "size mismatch for {name}");
    }
    unsafe {
        check::<ffi::ChiakiLog>(
            size_of::<ffi::ChiakiLog>(),
            shim::libchiaki_sizeof_ChiakiLog(),
            "ChiakiLog",
        );
        check::<ffi::ChiakiDiscovery>(
            size_of::<ffi::ChiakiDiscovery>(),
            shim::libchiaki_sizeof_ChiakiDiscovery(),
            "ChiakiDiscovery",
        );
        check::<ffi::ChiakiDiscoveryThread>(
            size_of::<ffi::ChiakiDiscoveryThread>(),
            shim::libchiaki_sizeof_ChiakiDiscoveryThread(),
            "ChiakiDiscoveryThread",
        );
        check::<ffi::ChiakiRegist>(
            size_of::<ffi::ChiakiRegist>(),
            shim::libchiaki_sizeof_ChiakiRegist(),
            "ChiakiRegist",
        );
        check::<ffi::ChiakiConnectInfo>(
            size_of::<ffi::ChiakiConnectInfo>(),
            shim::libchiaki_sizeof_ChiakiConnectInfo(),
            "ChiakiConnectInfo",
        );
        check::<ffi::ChiakiRegistInfo>(
            size_of::<ffi::ChiakiRegistInfo>(),
            shim::libchiaki_sizeof_ChiakiRegistInfo(),
            "ChiakiRegistInfo",
        );
        check::<ffi::ChiakiControllerState>(
            size_of::<ffi::ChiakiControllerState>(),
            shim::libchiaki_sizeof_ChiakiControllerState(),
            "ChiakiControllerState",
        );
        check::<ffi::ChiakiControllerTouch>(
            size_of::<ffi::ChiakiControllerTouch>(),
            shim::libchiaki_sizeof_ChiakiControllerTouch(),
            "ChiakiControllerTouch",
        );
        check::<ffi::ChiakiEvent>(
            size_of::<ffi::ChiakiEvent>(),
            shim::libchiaki_sizeof_ChiakiEvent(),
            "ChiakiEvent",
        );
        check::<ffi::ChiakiQuitEvent>(
            size_of::<ffi::ChiakiQuitEvent>(),
            shim::libchiaki_sizeof_ChiakiQuitEvent(),
            "ChiakiQuitEvent",
        );
        check::<ffi::ChiakiKeyboardEvent>(
            size_of::<ffi::ChiakiKeyboardEvent>(),
            shim::libchiaki_sizeof_ChiakiKeyboardEvent(),
            "ChiakiKeyboardEvent",
        );
        check::<ffi::ChiakiRumbleEvent>(
            size_of::<ffi::ChiakiRumbleEvent>(),
            shim::libchiaki_sizeof_ChiakiRumbleEvent(),
            "ChiakiRumbleEvent",
        );
        check::<ffi::ChiakiTriggerEffectsEvent>(
            size_of::<ffi::ChiakiTriggerEffectsEvent>(),
            shim::libchiaki_sizeof_ChiakiTriggerEffectsEvent(),
            "ChiakiTriggerEffectsEvent",
        );
        check::<ffi::ChiakiVideoFecFailureEvent>(
            size_of::<ffi::ChiakiVideoFecFailureEvent>(),
            shim::libchiaki_sizeof_ChiakiVideoFecFailureEvent(),
            "ChiakiVideoFecFailureEvent",
        );
        check::<ffi::ChiakiDiscoveryHost>(
            size_of::<ffi::ChiakiDiscoveryHost>(),
            shim::libchiaki_sizeof_ChiakiDiscoveryHost(),
            "ChiakiDiscoveryHost",
        );
        check::<ffi::ChiakiDiscoveryPacket>(
            size_of::<ffi::ChiakiDiscoveryPacket>(),
            shim::libchiaki_sizeof_ChiakiDiscoveryPacket(),
            "ChiakiDiscoveryPacket",
        );
        check::<ffi::ChiakiRegisteredHost>(
            size_of::<ffi::ChiakiRegisteredHost>(),
            shim::libchiaki_sizeof_ChiakiRegisteredHost(),
            "ChiakiRegisteredHost",
        );
        check::<ffi::ChiakiRegistEvent>(
            size_of::<ffi::ChiakiRegistEvent>(),
            shim::libchiaki_sizeof_ChiakiRegistEvent(),
            "ChiakiRegistEvent",
        );
        check::<ffi::ChiakiAudioHeader>(
            size_of::<ffi::ChiakiAudioHeader>(),
            shim::libchiaki_sizeof_ChiakiAudioHeader(),
            "ChiakiAudioHeader",
        );
        check::<ffi::ChiakiAudioSink>(
            size_of::<ffi::ChiakiAudioSink>(),
            shim::libchiaki_sizeof_ChiakiAudioSink(),
            "ChiakiAudioSink",
        );
        check::<ffi::ChiakiCtrlDisplaySink>(
            size_of::<ffi::ChiakiCtrlDisplaySink>(),
            shim::libchiaki_sizeof_ChiakiCtrlDisplaySink(),
            "ChiakiCtrlDisplaySink",
        );
        check::<ffi::ChiakiDiscoveryServiceOptions>(
            size_of::<ffi::ChiakiDiscoveryServiceOptions>(),
            shim::libchiaki_sizeof_ChiakiDiscoveryServiceOptions(),
            "ChiakiDiscoveryServiceOptions",
        );
        check::<ffi::ChiakiFeedbackHistoryBuffer>(
            size_of::<ffi::ChiakiFeedbackHistoryBuffer>(),
            shim::libchiaki_sizeof_ChiakiFeedbackHistoryBuffer(),
            "ChiakiFeedbackHistoryBuffer",
        );
        check::<ffi::ChiakiFeedbackHistoryEvent>(
            size_of::<ffi::ChiakiFeedbackHistoryEvent>(),
            shim::libchiaki_sizeof_ChiakiFeedbackHistoryEvent(),
            "ChiakiFeedbackHistoryEvent",
        );
        check::<ffi::ChiakiHolepunchRegistInfo>(
            size_of::<ffi::ChiakiHolepunchRegistInfo>(),
            shim::libchiaki_sizeof_ChiakiHolepunchRegistInfo(),
            "ChiakiHolepunchRegistInfo",
        );
        check::<ffi::ChiakiHolepunchDeviceInfo>(
            size_of::<ffi::ChiakiHolepunchDeviceInfo>(),
            shim::libchiaki_sizeof_ChiakiHolepunchDeviceInfo(),
            "ChiakiHolepunchDeviceInfo",
        );
        check::<ffi::ChiakiConnectVideoProfile>(
            size_of::<ffi::ChiakiConnectVideoProfile>(),
            shim::libchiaki_sizeof_ChiakiConnectVideoProfile(),
            "ChiakiConnectVideoProfile",
        );
        // 大对象走垫片分配, 这里只确认垫片值非零、对齐合法。
        for (name, s, a) in [
            (
                "ChiakiSession",
                shim::libchiaki_sizeof_ChiakiSession(),
                shim::libchiaki_alignof_ChiakiSession(),
            ),
            (
                "ChiakiDiscoveryService",
                shim::libchiaki_sizeof_ChiakiDiscoveryService(),
                shim::libchiaki_alignof_ChiakiDiscoveryService(),
            ),
        ] {
            assert!(s > 0, "{name} size is zero!");
            assert!(a.is_power_of_two(), "{name} align suspicious: {a}");
            assert!(s % a == 0, "{name} size {s} not multiple of align {a}");
        }
    }
}
