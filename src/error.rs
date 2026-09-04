//! chiaki 错误码。

use std::ffi::CStr;
use std::fmt;

use crate::ffi;

/// chiaki 错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub ffi::ChiakiErrorCode);

impl Error {
    pub fn code(self) -> ffi::ChiakiErrorCode {
        self.0
    }

    /// `chiaki_error_string` 的 Rust 字符串版本。
    pub fn message(self) -> String {
        unsafe {
            let p = ffi::chiaki_error_string(self.0);
            if p.is_null() {
                format!("unknown chiaki error {:?}", self.0 as u32)
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for Error {}

pub(crate) fn cvt(code: ffi::ChiakiErrorCode) -> Result<(), Error> {
    if code == ffi::ChiakiErrorCode::CHIAKI_ERR_SUCCESS {
        Ok(())
    } else {
        Err(Error(code))
    }
}
