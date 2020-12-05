use futures_channel::mpsc::TryRecvError;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(s: &str) -> Self {
        Error {
            message: String::from(s),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<qapi::ExecuteError> for Error {
    fn from(error: qapi::ExecuteError) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<TryRecvError> for Error {
    fn from(error: TryRecvError) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<nix::Error> for Error {
    fn from(error: nix::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<rtnetlink::Error> for Error {
    fn from(error: rtnetlink::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(error: std::num::ParseIntError) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<tinytemplate::error::Error> for Error {
    fn from(error: tinytemplate::error::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
