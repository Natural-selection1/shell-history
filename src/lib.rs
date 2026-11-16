//! Byte-preserving parser for Zsh history files.
//!
//! Parse with the known shell, filter decoded command bytes, and copy original
//! records without re-encoding their headers, escapes, or line endings:
//!
//! ```no_run
//! use zsh_history::{ZshParser, ParseOutcome};
//! use std::{fs::File, io::{self, BufReader, Write}};
//!
//! # fn main() -> io::Result<()> {
//! let path = std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME"))
//!     .join(".zsh_history");
//! let input = BufReader::new(File::open(path)?);
//! let mut output = io::stdout().lock();
//! for outcome in ZshParser::new().parse(input) {
//!     match outcome {
//!         ParseOutcome::Entry(entry) if entry.command.starts_with(b"git ") => {
//!             output.write_all(&entry.source.raw)?;
//!         }
//!         ParseOutcome::Entry(_) => {}
//!         ParseOutcome::Skipped { source, reason } => {
//!             eprintln!("{:?}: {reason}", source.span);
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Commands are bytes; [`HistoryEntry::command_str`] validates UTF-8 and
//! [`HistoryEntry::command_lossy`] is a display-only conversion. Successful
//! records retain exact source bytes. Corrupt history and I/O errors terminate the iterator
//! after one error outcome.

pub mod entry;
pub mod error;
mod lines;
pub mod parser;
pub mod zsh;

pub use entry::{HistoryEntry, SourceRecord, Timing};
pub use error::ParseError;
pub use parser::ParseOutcome;
pub use zsh::ZshParser;
