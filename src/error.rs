use std::{io, fmt::Debug};

use tokio::{net::TcpStream, io::{AsyncRead, AsyncWrite}};
use webparse::{WebError, BinaryMut};

// #[derive(Debug)]
pub enum AppError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    IoError(io::Error),
    WebError(WebError),
    /// 该错误发生协议不可被解析, 则尝试下一个协议
    Continue((Option<BinaryMut>, T)),
    VerifyFail,
    UnknownHost,
    SizeNotMatch,
    TooShort,
    ProtErr,
    ProtNoSupport,
    Extension(&'static str)
}

impl<T> AppError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    pub fn extension(value: &'static str) -> AppError<T> {
        AppError::Extension(value)
    }

    pub fn is_weberror(&self) -> bool {
        match self {
            AppError::WebError(_) => true,
            _ => false,
        }
    }
    pub fn to_type<B>(self) -> AppError<B> 
    where B : AsyncRead + AsyncWrite + Unpin{
        match self {
            AppError::IoError(e) => AppError::IoError(e),
            AppError::WebError(e) => AppError::WebError(e),
            AppError::Continue(_) => unreachable!("continue can't convert"),
            AppError::VerifyFail => AppError::VerifyFail,
            AppError::UnknownHost => AppError::UnknownHost,
            AppError::SizeNotMatch => AppError::SizeNotMatch,
            AppError::TooShort => AppError::TooShort,
            AppError::ProtErr => AppError::ProtErr,
            AppError::ProtNoSupport => AppError::ProtNoSupport,
            AppError::Extension(s) => AppError::Extension(s),
        }
    }


}
 
pub type AppResult<T> = Result<T, AppError<TcpStream>>;
pub type AppTypeResult<T, B> = Result<T, AppError<B>>;


impl<T> From<io::Error> for AppError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn from(value: io::Error) -> Self {
        AppError::IoError(value)
    }
}

impl<T> From<WebError> for AppError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn from(value: WebError) -> Self {
        AppError::WebError(value)
    }
}

impl<T> Debug for AppError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(arg0) => f.debug_tuple("IoError").field(arg0).finish(),
            Self::WebError(arg0) => f.debug_tuple("WebError").field(arg0).finish(),
            Self::Continue(_arg0) => f.debug_tuple("Continue").finish(),
            Self::VerifyFail => write!(f, "VerifyFail"),
            Self::UnknownHost => write!(f, "UnknownHost"),
            Self::SizeNotMatch => write!(f, "SizeNotMatch"),
            Self::TooShort => write!(f, "TooShort"),
            Self::ProtErr => write!(f, "ProtErr"),
            Self::ProtNoSupport => write!(f, "ProtNoSupport"),
            Self::Extension(arg0) => f.debug_tuple("Extension").field(arg0).finish(),
        }
    }
}