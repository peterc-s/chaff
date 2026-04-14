//! Errors for the Chaff dataset crate.

// These aren't testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt, io, path::PathBuf};

/// Errors while parsing datasets.
#[derive(Debug)]
pub enum ParseError {
    /// An error from [`std::io`].
    Io(io::Error),

    /// Invalid file name (for datasets with file name importance).
    InvalidFileName {
        /// The file which is improperly named.
        file: PathBuf,

        /// The message from the parser.
        message: String,
    },

    /// Expected to be passed a directory, but was not.
    NotADirectory(PathBuf),

    /// File format is incorrect for the given parser.
    InvalidFormat {
        /// The file where the invalid formatting was found.
        file: PathBuf,

        /// The line number in the file where invalid formatting was found.
        line: usize,

        /// The message from the parser.
        message: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::InvalidFileName { file, message } => write!(
                f,
                "error parsing filename for {}: {message}",
                file.display()
            ),
            Self::NotADirectory(path) => write!(f, "path is not a directory: {}", path.display()),
            Self::InvalidFormat {
                file,
                line,
                message,
            } => write!(
                f,
                "invalid format in {} at line {line}: {message}",
                file.display()
            ),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::NotADirectory(_) | Self::InvalidFormat { .. } | Self::InvalidFileName { .. } => {
                None
            }
        }
    }
}
