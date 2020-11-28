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
