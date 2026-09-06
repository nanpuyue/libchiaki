# API 覆盖清单

对照对象: chiaki-ng v1.10.0 (`lib/include/chiaki/*.h`, 294 个 `CHIAKI_EXPORT`)。
本文件回答三个问题: 哪些 API 已有安全封装、哪些属于内部构件刻意不封装、
哪些因编译脚本禁用了依赖而当前不可用 (未来做功能开关的候选)。

最后更新: 与本次 orientation/opus/log-sniffer 补齐同一提交。

## 已封装 (安全 API)

| 头文件 | 封装位置 | 说明 |
|---|---|---|
| common.h | `common.rs` | lib_init, target/codec/quit-reason 判定与字符串 |
| session.h (session 部分) | `session.rs` | Session 全生命周期 + 事件/视频/音频/haptics/display 回调、login pin、keyboard、microphone、IDR |
| session.h (杂项纯函数) | `common.rs` | rp_version_string/parse、connect_video_profile_preset (`connect.rs`) |
| session.h (static inline) | `session.rs` | ctrl_set_display_sink (`set_display_sink`) |
| videoreceiver.h | `session.rs` | set/get_waiting_for_idr、get_frames_lost_total (经 `session.stream_connection.video_receiver`) |
| controller.h | `controller.rs` | ControllerState: idle/set_idle/equals/or/union、touch、set_button/touchpad; 按钮位常量 |
| log.h | `log.rs` | Log (RAII, 回调/stdout)、log_level_char、set_level; **LogSniffer** (截流缓冲) |
| discovery.h / discoveryservice.h | `discovery.rs` | Discovery (send/wakeup)、DiscoveryPacket (search/wakeup/fmt)、DiscoveryThread、DiscoveryService (含 host 解析, is_ps5/版本) |
| regist.h | `regist.rs` | RegistInfo (builder)、Regist (start/stop + 事件回调)、RegisteredHost 解析 |
| remote/holepunch.h | `holepunch.rs` | 设备列表、DUID、HolepunchSession 全流程 (create/start/punch/upnp/offer/port guessing/STUN/sock/regist info) |
| feedback.h | `feedback.rs` | HistoryEvent / HistoryBuffer (init/push/format/fini) |
| orientation.h | `orientation.rs` | Orientation、OrientationTracker、AccelNewZero (体感全链) |
| opusencoder.h / opusdecoder.h | `opus.rs` | OpusEncoder (frame 编码后自动发送)、OpusDecoder (set_cb + sink) |
| audio.h | `session.rs` | AudioHeader: set/frame_bytes/load/save |
| base64.h | `common.rs` | base64_encode/decode |
| random.h | `common.rs` | random_32、random_bytes_crypt |
| time.h | `common.rs` | time_now_monotonic_us/ms |
| sock.h | `sock.rs` | socket_set_nonblock |
| takion.h (AV packet 解析) | `session.rs` | 视频回调内部使用 TakionAVPacket 解析结果 |

## 刻意不封装 (内部构件)

这些是 chiaki 会话内部机件的"裸零件", 单独暴露无法安全使用
(生命周期与 session/takion 强耦合, 且需要手写缓冲区管理)。
参考实现 chiaki-ng GUI 也从不直接调用它们 — 全部经 Session 间接触达:

- `takion.h` 除 AV packet 外的连接/发送/拥塞控制
- `remote/rudp.h`、`remote/rudpsendbuffer.h`、`takionsendbuffer.h`
- `gkcrypt.h`、`rpcrypt.h`、`ecdh.h` (密钥派生/流加密 — session 内部自持)
- `fec.h`、`reorderqueue.h`、`frameprocessor.h`、`packetstats.h`
- `congestioncontrol.h`、`streamconnection.h` (经 Session 暴露其功能)
- `audio_receiver.h`、`audio_sender.h`、`feedbacksender.h`、`feedback_state_format_*`
- `stoppipe.h`、`thread.h`、`mutex/cond` (bool_pred_cond) — Rust 标准库对应物更优
- `bitstream.h`、`key_state.h`、`seqnum.h`、`aligned_alloc/free`
- `http.h` (header/response 解析) — GUI 用 Qt/curl 自行处理 PSN 请求;
  如后续要在无 Qt 场景复刻 PSN 注册流程再评估
- `ctrl.h` 独立控制通道 (ctrl_init/start/send_message/...) — GUI 仅经
  Session 内嵌的 ctrl 使用; 独立用法尚未出现, 出现后再封装
- `senkusha.h` (连接测试) — GUI 未直接调用 (经 session 内部); 若要
  做"仅测速不串流"的功能再封装
- `launchspec.h` — launchspec_format 仅为调试输出

## 受编译禁用影响 (功能开关候选)

编译脚本 (build-libchiaki.sh) 为裁剪依赖关闭了以下 chiaki 选项,
对应的 API 在当前产物中**不存在**, 绑定无法生成:

| 被禁选项 | 缺失的 API | GUI 是否使用 | 备注 |
|---|---|---|---|
| `CHIAKI_ENABLE_FFMPEG_DECODER=OFF` | `ffmpegdecoder.h`: ffmpeg_decoder_init/fini/pull_frame | **是** (视频解码主路径) | GUI 默认用 FFmpeg 软解; Rust 侧若要开箱即用需要此开关 + FFmpeg 依赖, 或由绑定层接 libav/rust 分支替代 |
| `CHIAKI_ENABLE_PI_DECODER` (默认 OFF) | `pidecoder.h`: pi_decoder_* | 否 (树莓派硬件解码) | 低优先级 |
| `CHIAKI_ENABLE_SETSU=OFF` | setsu (Steam Deck 触摸/陀螺输入) | 是 (仅 Steam Deck) | 与 libchiaki 链接无直接关系, GUI 层功能 |
| `CHIAKI_ENABLE_STEAMDECK_NATIVE=OFF` | steamdeck haptics/led 等 | 是 (仅 Steam Deck) | 同上 |
| `CHIAKI_ENABLE_SPEEX=OFF` | 语音处理 (回声消除) | 否 | 麦克风链路的可选增强 |
| curl 依赖裁剪 (见 README) | 无 API 缺失 — holepunch 的 HTTP/WS 路径已保留 | — | 只裁了 curl 自身的可选依赖 |

## 与上游行为不一致处 (忠实还原的已知 bug)

- `chiaki_audio_header_save` 写入 `buf[0]=bits, buf[1]=channels`, 而
  `chiaki_audio_header_load` 按 `buf[0]=channels, buf[1]=bits` 读取 —
  上游即如此 (save/load 顺序互换)。封装不做矫正, 测试
  (`tests/api_surface.rs::audio_header_roundtrip`) 记录了该行为。

## chiaki-ng GUI 高频操作对照 (便利性核验)

GUI (gui/src) 调用频次最高的 libchiaki API 及其 Rust 对应:

| C API (GUI 调用次数) | Rust |
|---|---|
| controller_state_set_idle (12) | `ControllerState::set_idle` |
| time_now_monotonic_us (11) | `time_now_monotonic_us` |
| controller_state_or (10) | `ControllerState::union_with` |
| target_is_ps5 (9) | `target_is_ps5` |
| orientation_tracker_update (8) | `OrientationTracker::update` |
| controller_state_start/set/stop_touch (16) | 同名方法 |
| accel_new_zero_set_active/inactive (13) | `AccelNewZero::set_active/set_inactive` |
| error_string (6) | `error_string` |
| log_init / log_cb_print (8) | `Log::new` / `Log::print_to_stdout` |
| log_sniffer_* (8) | `LogSniffer` |
| opus_encoder/decoder_* (12) | `OpusEncoder` / `OpusDecoder` |
| discovery_service_* / wakeup (7) | `DiscoveryService` / `Discovery::wakeup` |
| holepunch_session_* (9) | `HolepunchSession` |
| video_receiver_set_waiting_for_idr / get_frames_lost_total | `Session::video_receiver_*` |
