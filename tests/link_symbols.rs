//! 符号完整性检查: 通过对每个公开入口取地址, 强制链接器在
//! `libchiaki.a` 中解析它们。符号缺失时 cargo test 的**链接阶段**
//! 直接失败 — 无需 nm, 跨平台一致 (替代 build-libchiaki.sh 里的
//! nm 检查)。
//!
//! 清单对应 docs/API_COVERAGE.md 的"已封装"部分; 新增封装时在此
//! 补一行即可。函数项到 `usize` 的转换在链接完成后恒为非零,
//! 断言本身只是形式 — 真正的检查发生在链接期。

use libchiaki::ffi;

#[test]
fn public_symbols_resolve_at_link_time() {
    let mut list: Vec<*const ()> = vec![
        // lib 生命周期
        ffi::chiaki_lib_init as *const (),
        // session
        ffi::chiaki_session_init as *const (),
        ffi::chiaki_session_start as *const (),
        ffi::chiaki_session_stop as *const (),
        ffi::chiaki_session_join as *const (),
        ffi::chiaki_session_fini as *const (),
        ffi::chiaki_session_set_controller_state as *const (),
        ffi::chiaki_session_set_login_pin as *const (),
        ffi::chiaki_session_set_event_cb as *const (),
        ffi::chiaki_session_set_video_sample_cb as *const (),
        ffi::chiaki_session_set_audio_sink as *const (),
        ffi::chiaki_session_set_haptics_sink as *const (),
        ffi::chiaki_session_request_idr as *const (),
        ffi::chiaki_session_go_home as *const (),
        ffi::chiaki_session_goto_bed as *const (),
        ffi::chiaki_session_toggle_microphone as *const (),
        ffi::chiaki_session_connect_microphone as *const (),
        ffi::chiaki_session_ctrl_set_display_sink as *const (),
        // 杂项纯函数
        ffi::chiaki_connect_video_profile_preset as *const (),
        ffi::chiaki_rp_version_string as *const (),
        ffi::chiaki_rp_version_parse as *const (),
        ffi::chiaki_rp_application_reason_string as *const (),
        ffi::chiaki_quit_reason_string as *const (),
        ffi::chiaki_error_string as *const (),
        // controller
        ffi::chiaki_controller_state_set_idle as *const (),
        ffi::chiaki_controller_state_or as *const (),
        ffi::chiaki_controller_state_equals as *const (),
        ffi::chiaki_controller_state_start_touch as *const (),
        ffi::chiaki_controller_state_stop_touch as *const (),
        ffi::chiaki_controller_state_set_touch_pos as *const (),
        // log + sniffer
        ffi::chiaki_log_init as *const (),
        ffi::chiaki_log_set_level as *const (),
        ffi::chiaki_log_level_char as *const (),
        ffi::chiaki_log_cb_print as *const (),
        ffi::chiaki_log_sniffer_init as *const (),
        ffi::chiaki_log_sniffer_fini as *const (),
        // discovery
        ffi::chiaki_discovery_init as *const (),
        ffi::chiaki_discovery_send as *const (),
        ffi::chiaki_discovery_wakeup as *const (),
        ffi::chiaki_discovery_fini as *const (),
        ffi::chiaki_discovery_service_init as *const (),
        ffi::chiaki_discovery_service_fini as *const (),
        ffi::chiaki_discovery_thread_start as *const (),
        ffi::chiaki_discovery_thread_stop as *const (),
        ffi::chiaki_discovery_host_is_ps5 as *const (),
        ffi::chiaki_discovery_host_state_string as *const (),
        // regist
        ffi::chiaki_regist_start as *const (),
        ffi::chiaki_regist_stop as *const (),
        // holepunch
        ffi::chiaki_holepunch_list_devices as *const (),
        ffi::chiaki_holepunch_generate_client_device_uid as *const (),
        ffi::chiaki_holepunch_session_init as *const (),
        ffi::chiaki_holepunch_session_create as *const (),
        ffi::chiaki_holepunch_session_start as *const (),
        ffi::chiaki_holepunch_session_punch_hole as *const (),
        ffi::chiaki_holepunch_session_fini as *const (),
        ffi::chiaki_holepunch_upnp_discover as *const (),
        ffi::chiaki_get_regist_info as *const (),
        // feedback
        ffi::chiaki_feedback_history_buffer_init as *const (),
        ffi::chiaki_feedback_history_buffer_push as *const (),
        ffi::chiaki_feedback_history_buffer_format as *const (),
        ffi::chiaki_feedback_history_event_set_button as *const (),
        // orientation
        ffi::chiaki_orientation_init as *const (),
        ffi::chiaki_orientation_update as *const (),
        ffi::chiaki_orientation_tracker_init as *const (),
        ffi::chiaki_orientation_tracker_update as *const (),
        ffi::chiaki_orientation_tracker_apply_to_controller_state as *const (),
        ffi::chiaki_accel_new_zero_set_active as *const (),
        ffi::chiaki_accel_new_zero_set_inactive as *const (),
        // video receiver
        ffi::chiaki_video_receiver_set_waiting_for_idr as *const (),
        ffi::chiaki_video_receiver_get_waiting_for_idr as *const (),
        ffi::chiaki_video_receiver_get_frames_lost_total as *const (),
        // audio header
        ffi::chiaki_audio_header_set as *const (),
        ffi::chiaki_audio_header_load as *const (),
        ffi::chiaki_audio_header_save as *const (),
        // 杂项工具
        ffi::chiaki_base64_encode as *const (),
        ffi::chiaki_base64_decode as *const (),
        ffi::chiaki_random_32 as *const (),
        ffi::chiaki_random_bytes_crypt as *const (),
        ffi::chiaki_time_now_monotonic_us as *const (),
        ffi::chiaki_socket_set_nonblock as *const (),
    ];
    // opus 符号只在 feature 开启时存在; 与其余符号一样进同一个容器,
    // 下面的 is_null 断言一视同仁地覆盖。
    #[cfg(feature = "opus")]
    list.extend([
        ffi::chiaki_opus_encoder_init as *const (),
        ffi::chiaki_opus_encoder_header as *const (),
        ffi::chiaki_opus_encoder_frame as *const (),
        ffi::chiaki_opus_encoder_fini as *const (),
        ffi::chiaki_opus_decoder_init as *const (),
        ffi::chiaki_opus_decoder_get_sink as *const (),
        ffi::chiaki_opus_decoder_fini as *const (),
    ]);
    // 链接成功后地址恒非空, is_null 只是形式上的最后防线 —
    // 真正的检查发生在本测试的链接阶段。
    assert!(!list.iter().any(|p| p.is_null()));
}
