//! When serializing or deserializing VICI goes wrong.

use core::result;
use std::{
    error,
    fmt::{self, Debug, Display},
    io,
};

use serde::{de, ser};

/// A structure representing all possible errors that can occur when serializing or deserializing VICI data.
pub struct Error {
    err: Box<ErrorImpl>,
}

struct ErrorImpl {
    code: ErrorCode,
    input: Option<u8>,
    pos: Option<usize>,
}

/// Alias for a `Result` with the error type `serde_vici::Error`.
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// Zero-based byte index at which the error was detected.
    pub fn position(&self) -> Option<usize> {
        self.err.pos
    }

    /// Categorizes the cause of this error.
    ///
    /// - `Category::Io` - failure to read or write bytes on an IO stream
    /// - `Category::Data` - invalid data
    /// - `Category::Eof` - unexpected end of the input data
    pub fn classify(&self) -> Category {
        match self.err.code {
            ErrorCode::Io(_) => Category::Io,
            ErrorCode::Message(_) | ErrorCode::InvalidUnicodeCodePoint => Category::Data,
            ErrorCode::EofWhileParsingElementType | ErrorCode::EofWhileParsingKey | ErrorCode::EofWhileParsingValue => Category::Eof,
        }
    }

    /// Returns true if this error was caused by a failure to read or write bytes on an IO stream.
    pub fn is_io(&self) -> bool {
        self.classify() == Category::Io
    }

    /// Returns true if this error was caused by invalid data.
    pub fn id_data(&self) -> bool {
        self.classify() == Category::Data
    }

    /// Returns true if this error was caused by prematurely reaching the end of the input data.
    pub fn is_eof(&self) -> bool {
        self.classify() == Category::Eof
    }

    pub(crate) fn io(e: io::Error, pos: Option<usize>) -> Self {
        Self {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Io(e),
                input: None,
                pos,
            }),
        }
    }

    pub(crate) fn data(code: ErrorCode, input: Option<u8>, pos: Option<usize>) -> Self {
        Self {
            err: Box::new(ErrorImpl { code, input, pos }),
        }
    }
}

/// Categorizes the cause of a `serde_vici::Error`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Category {
    /// The error was caused by a failure to read or write bytes on an IO stream.
    Io,

    /// The error was caused by invalid data.
    Data,

    /// The error was caused by prematurely reaching the end of the input data.
    Eof,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::io(e, None)
    }
}

impl From<Error> for io::Error {
    /// Convert a `serde_vici::Error` into an `io::Error`.
    ///
    /// VICI data errors are turned into `InvalidData` IO errors.
    /// EOF errors are turned into `UnexpectedEof` IO errors.
    fn from(e: Error) -> Self {
        match e.classify() {
            Category::Io => {
                if let ErrorCode::Io(e) = e.err.code {
                    e
                } else {
                    unreachable!()
                }
            },
            Category::Data => io::Error::new(io::ErrorKind::InvalidData, e),
            Category::Eof => io::Error::new(io::ErrorKind::UnexpectedEof, e),
        }
    }
}

pub(crate) enum ErrorCode {
    /// Some IO error occurred while serializing or deserializing.
    Io(io::Error),

    /// Catchall for invalid data error messages.
    Message(String),

    /// EOF while parsing an element type.
    EofWhileParsingElementType,

    /// EOF while parsing a key.
    EofWhileParsingKey,

    /// EOF while parsing a value.
    EofWhileParsingValue,

    /// Invalid unicode code point.
    InvalidUnicodeCodePoint,
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ErrorCode::Io(ref err) => Display::fmt(err, f),
            ErrorCode::Message(ref msg) => f.write_str(msg),
            ErrorCode::EofWhileParsingElementType => f.write_str("EOF while parsing element type"),
            ErrorCode::EofWhileParsingKey => f.write_str("EOF while parsing key"),
            ErrorCode::EofWhileParsingValue => f.write_str("EOF while parsing value"),
            ErrorCode::InvalidUnicodeCodePoint => f.write_str("invalid unicode code point"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.err.code {
            ErrorCode::Io(ref err) => Some(err),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.err, f)
    }
}

impl Display for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.code, f)?;

        if let Some(input) = self.input {
            f.write_fmt(format_args!(" 0x{:x}", input))?;
        }

        if let Some(pos) = self.pos {
            f.write_fmt(format_args!(" at position {}", pos))?;
        }

        Ok(())
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error({:?})", self.err.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        make_error(msg.to_string(), None)
    }
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        make_error(msg.to_string(), None)
    }
}

fn make_error(msg: String, pos: Option<usize>) -> Error {
    let input = None;
    let code = ErrorCode::Message(msg);
    Error {
        err: Box::new(ErrorImpl { code, input, pos }),
    }
}
