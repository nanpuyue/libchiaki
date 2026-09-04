// Tiny C shim for libchiaki (compiled by build.rs).
//
// Two jobs, both of which need the REAL C compiler + headers:
//
//  1. sizeof/_Alignof for structs whose bindgen layout cannot be trusted
//     (bindgen 0.71 emits opaque placeholders for structs whose first
//     encounter in the translation unit is a forward reference, e.g.
//     ChiakiSession via streamconnection.h, RudpMessage, nanopb internals,
//     _RTL_CRITICAL_SECTION, ...). Rust allocates Session/DiscoveryService
//     from these values and never lays them out itself.
//  2. Thin wrappers mirroring chiaki's `static inline` helpers
//     (chiaki_session_set_event_cb et al.), which live in headers only
//     and are NOT exported by libchiaki.a, so Rust cannot link them
//     directly.
#include <stddef.h>
#include <chiaki/audio.h>
#include <chiaki/audioreceiver.h>
#include <chiaki/controller.h>
#include <chiaki/discovery.h>
#include <chiaki/discoveryservice.h>
#include <chiaki/feedback.h>
#include <chiaki/log.h>
#include <chiaki/regist.h>
#include <chiaki/session.h>

#define LIBCHIAKI_LAYOUT(T)                                 \
    size_t libchiaki_sizeof_##T(void) { return sizeof(T); } \
    size_t libchiaki_alignof_##T(void) { return _Alignof(T); }

LIBCHIAKI_LAYOUT(ChiakiSession)
LIBCHIAKI_LAYOUT(ChiakiDiscoveryService)
LIBCHIAKI_LAYOUT(ChiakiLog)
LIBCHIAKI_LAYOUT(ChiakiDiscovery)
LIBCHIAKI_LAYOUT(ChiakiDiscoveryThread)
LIBCHIAKI_LAYOUT(ChiakiRegist)
LIBCHIAKI_LAYOUT(ChiakiConnectInfo)
LIBCHIAKI_LAYOUT(ChiakiRegistInfo)
LIBCHIAKI_LAYOUT(ChiakiControllerState)
LIBCHIAKI_LAYOUT(ChiakiControllerTouch)
LIBCHIAKI_LAYOUT(ChiakiEvent)
LIBCHIAKI_LAYOUT(ChiakiQuitEvent)
LIBCHIAKI_LAYOUT(ChiakiKeyboardEvent)
LIBCHIAKI_LAYOUT(ChiakiRumbleEvent)
LIBCHIAKI_LAYOUT(ChiakiTriggerEffectsEvent)
LIBCHIAKI_LAYOUT(ChiakiVideoFecFailureEvent)
LIBCHIAKI_LAYOUT(ChiakiDiscoveryHost)
LIBCHIAKI_LAYOUT(ChiakiDiscoveryPacket)
LIBCHIAKI_LAYOUT(ChiakiRegisteredHost)
LIBCHIAKI_LAYOUT(ChiakiRegistEvent)
LIBCHIAKI_LAYOUT(ChiakiAudioHeader)
LIBCHIAKI_LAYOUT(ChiakiAudioSink)
LIBCHIAKI_LAYOUT(ChiakiCtrlDisplaySink)
LIBCHIAKI_LAYOUT(ChiakiDiscoveryServiceOptions)
LIBCHIAKI_LAYOUT(ChiakiFeedbackHistoryBuffer)
LIBCHIAKI_LAYOUT(ChiakiFeedbackHistoryEvent)
LIBCHIAKI_LAYOUT(ChiakiHolepunchRegistInfo)
LIBCHIAKI_LAYOUT(ChiakiHolepunchDeviceInfo)
LIBCHIAKI_LAYOUT(ChiakiConnectVideoProfile)

void libchiaki_session_set_event_cb(ChiakiSession *s, ChiakiEventCallback cb, void *user)
{
    chiaki_session_set_event_cb(s, cb, user);
}

void libchiaki_session_set_video_sample_cb(ChiakiSession *s, ChiakiVideoSampleCallback cb, void *user)
{
    chiaki_session_set_video_sample_cb(s, cb, user);
}

void libchiaki_session_set_audio_sink(ChiakiSession *s, const ChiakiAudioSink *sink)
{
    chiaki_session_set_audio_sink(s, (ChiakiAudioSink *)sink);
}

void libchiaki_session_set_haptics_sink(ChiakiSession *s, const ChiakiAudioSink *sink)
{
    chiaki_session_set_haptics_sink(s, (ChiakiAudioSink *)sink);
}

void libchiaki_session_set_display_sink(ChiakiSession *s, const ChiakiCtrlDisplaySink *sink)
{
    chiaki_session_ctrl_set_display_sink(s, (ChiakiCtrlDisplaySink *)sink);
}
