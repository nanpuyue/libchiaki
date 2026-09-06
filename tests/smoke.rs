//! 公开 API 冒烟测试 (链接验证 + 基本行为, 不需要真实 PS 主机)。
//!
//! 运行: CHIAKI_INSTALL_DIR 指向 build-libchiaki.sh 的产物,
//! `cargo test --target x86_64-pc-windows-gnu` (Windows, MINGW64 shell)。

use libchiaki::{
    ConnectInfo, ControllerState, DiscoveryPacket, Error, LOG_ALL, Log, ffi, lib_init,
};

#[test]
fn error_message() {
    let m = Error(ffi::ChiakiErrorCode::CHIAKI_ERR_TIMEOUT).message();
    assert!(!m.is_empty(), "error string must not be empty");
}

#[test]
fn lib_init_works() {
    lib_init().expect("chiaki_lib_init");
    // 幂等: 再调一次也不应失败。
    lib_init().expect("chiaki_lib_init again");
}

#[test]
fn log_and_controller() {
    let _log = Log::print_to_stdout(LOG_ALL);
    let st = ControllerState::idle();
    assert_eq!(st.0.buttons, 0);
    assert!(st.equals(&ControllerState::idle()));
}

#[test]
fn connect_info_builds() {
    let key = [0xabu8; 16];
    let mut info = ConnectInfo::new("192.168.1.2", &key, true).unwrap();
    info.set_video_preset(
        ffi::ChiakiVideoResolutionPreset::CHIAKI_VIDEO_RESOLUTION_PRESET_1080p,
        ffi::ChiakiVideoFPSPreset::CHIAKI_VIDEO_FPS_PRESET_60,
    );
    info.set_auto_regist(true);
}

#[test]
fn discovery_packet_format() {
    let p = DiscoveryPacket::new_search(false);
    assert!(!p.format().unwrap().is_empty());
    let p5 = DiscoveryPacket::new_search(true);
    assert!(!p5.format().unwrap().is_empty());
}
