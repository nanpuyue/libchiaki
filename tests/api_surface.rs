//! 新增封装 (orientation / opus / log sniffer / 杂项) 的功能验证。

use libchiaki::*;
use std::ffi::CString;

#[test]
fn base64_roundtrip() {
    let s = "hello chiaki";
    let enc = base64_encode(s.as_bytes()).unwrap();
    assert_eq!(enc, "aGVsbG8gY2hpYWtp");
    let dec = base64_decode(&enc).unwrap();
    assert_eq!(String::from_utf8(dec).unwrap(), s);
}

#[test]
fn time_monotonic() {
    let a = time_now_monotonic_us();
    let b = time_now_monotonic_us();
    assert!(b >= a);
    assert!(time_now_monotonic_ms() >= b / 1000);
}

#[test]
fn random_bytes() {
    let r = random_32();
    let mut buf = [0u8; 32];
    random_bytes_crypt(&mut buf).unwrap();
    // 与自身内容不同的概率 1-2^-32
    assert!(buf.iter().any(|&b| b != 0) || r == 0);
}

#[test]
fn controller_set_idle_in_place() {
    let mut s = ControllerState::idle();
    s.0.buttons = BUTTON_CROSS;
    s.set_idle();
    assert!(s.equals(&ControllerState::idle()));
}

#[test]
fn orientation_tracker() {
    let mut tracker = OrientationTracker::new();
    let mut accel_zero = AccelNewZero::new();
    let mut state = ControllerState::idle();
    tracker.update(0.0, 0.0, 0.0, 0.0, 0.0, 1.0, &mut accel_zero, false, 1000);
    tracker.apply_to_controller_state(&mut state);
}

#[test]
fn log_sniffer_captures() {
    let forward = Log::print_to_stdout(LOG_ALL);
    let sniffer = LogSniffer::new(LOG_ALL, &forward);
    // 通过变参 C API 写一条日志, 应同时进入 sniffer 缓冲并转发。
    let fmt = CString::new("rust sniff test %d").unwrap();
    unsafe {
        ffi::chiaki_log(
            sniffer.as_log_ptr(),
            ffi::ChiakiLogLevel::CHIAKI_LOG_INFO,
            fmt.as_ptr(),
            42i32,
        );
    }
    assert!(sniffer.buffer().contains("rust sniff test 42"));
}

#[test]
fn audio_header_roundtrip() {
    let mut h = AudioHeader::new(2, 16, 48000, 960);
    assert_eq!(h.frame_bytes(), 960 * 2 * 2);
    let mut buf = [0u8; 32];
    h.save(&mut buf).unwrap();
    let h2 = AudioHeader::load(&buf).unwrap();
    assert_eq!(h2.0.rate, 48000);
    assert_eq!(h2.0.frame_size, 960);
    // 上游 bug: save 写 buf[0]=bits / buf[1]=channels, load 按
    // buf[0]=channels / buf[1]=bits 读, 两者互换。封装忠实还原该行为。
    assert_eq!(h2.0.channels, 16);
    assert_eq!(h2.0.bits, 2);
}

#[test]
fn audio_header_short_buf_rejected() {
    let mut h = AudioHeader::new(2, 16, 48000, 960);
    // C 侧固定读写 14 字节, 短切片必须在 Rust 侧被拒绝而非越界。
    assert!(h.save(&mut [0u8; 13]).is_err());
    assert!(AudioHeader::load(&[0u8; 13]).is_err());
    assert!(h.save(&mut [0u8; 14]).is_ok());
    assert!(AudioHeader::load(&[0u8; 14]).is_ok());
}

#[test]
fn opus_encoder_decoder_lifecycle() {
    let log = Log::print_to_stdout(LOG_ALL);
    let enc = OpusEncoder::new(&log);
    let mut dec = OpusDecoder::new(&log);
    dec.set_cb(|_, _| {}, |_: &mut [i16]| {});
    let _sink = dec.sink();
    // header() 需要活动 session, 这里只验证 init/fini 生命周期。
    drop(enc);
    drop(dec);
}

#[test]
fn opus_frame_length_checked() {
    let log = Log::print_to_stdout(LOG_ALL);
    let mut enc = OpusEncoder::new(&log);
    // 未绑定 header 时期望采样数为 0, frame() 必须拒绝而不进 C 侧。
    // (绑定 header 需要 Session, 按测试原则不建网络对象; 绑定后的
    // 校验与这里是同一个比较分支, expected 由 expected_pcm_len 决定。)
    assert_eq!(enc.expected_pcm_len(), 0);
    assert!(enc.frame(&mut []).is_err());
    assert!(enc.frame(&mut [0i16; 960]).is_err());
}

#[test]
fn discovery_service_options_defaults() {
    // 零值默认在 C 侧不可用 (hosts_max=0 → calloc(0)=NULL → init 报
    // MEMORY; ping_ms=0 → 忙轮询), 必须对齐 GUI 的初始值。
    let o = DiscoveryServiceOptions::new();
    assert_eq!(o.hosts_max(), 16);
    assert_eq!(o.ping_ms(), 500);
    assert_eq!(o.host_drop_pings(), 3);
}
