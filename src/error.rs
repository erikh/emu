use std::fmt;
use std::sync::mpsc::RecvError;

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

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        let errstr = error.to_string();
        Error { message: errstr }
    }
}

impl From<RecvError> for Error {
    fn from(error: RecvError) -> Self {
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
