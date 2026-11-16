use thiserror::Error;

/// An error encountered while reading a history stream.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("corrupt history file")]
    CorruptHistory,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
