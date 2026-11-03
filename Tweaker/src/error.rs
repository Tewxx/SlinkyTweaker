use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Jvm(String),
    NotFound { what: String, tried: String },
    UnexpectedNull { what: String },
    JavaException { during: String, message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Jvm(e) => write!(f, "{e}"),
            Error::NotFound { what, tried } if tried.is_empty() => {
                write!(f, "Not found: {what}")
            }
            Error::NotFound { what, tried } => {
                write!(f, "Not found: {what}\n\nTried:\n{tried}")
            }
            Error::UnexpectedNull { what } => write!(f, "{what} unexpectedly returned null."),
            Error::JavaException { during, message } => {
                write!(f, "Java exception during {during}:\n\n{message}")
            }
        }
    }
}
