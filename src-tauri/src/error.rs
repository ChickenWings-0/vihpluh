//! The error type that crosses the IPC boundary (ARCHITECTURE §7.1). Module errors are
//! `thiserror` enums and convert into [`AppError`] at the command boundary.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    NotFound,
    InvalidArgument,
    Io,
    Mpv,
    Db,
    Unsupported,
    Busy,
    Internal,
}

#[derive(Debug, Clone, Serialize, TS, thiserror::Error)]
#[error("{message}")]
#[ts(export)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), detail: None }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let code = if err.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        Self::new(code, err.to_string())
    }
}

impl From<crate::player::mpv::MpvError> for AppError {
    fn from(err: crate::player::mpv::MpvError) -> Self {
        Self::new(ErrorCode::Mpv, err.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
