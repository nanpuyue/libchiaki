//! socket 工具 (`chiaki/sock.h`)。

use crate::error::{Error, cvt};
use crate::ffi;

/// C: `chiaki_socket_set_nonblock` — 设置/取消非阻塞模式。
/// 典型用途: 打洞拿到的 socket (`HolepunchSession::socket`)。
pub fn set_nonblock(sock: ffi::chiaki_socket_t, nonblock: bool) -> Result<(), Error> {
    cvt(unsafe { ffi::chiaki_socket_set_nonblock(sock, nonblock) })
}
