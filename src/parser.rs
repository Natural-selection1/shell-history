use crate::{
    entry::{HistoryEntry, SourceRecord},
    error::ParseError,
};

/// Result of attempting to parse one logical entry.
///
/// Corrupt input and I/O failures terminate parsing after one error outcome.
#[derive(Debug)]
pub enum ParseOutcome {
    /// Successfully parsed entry.
    Entry(HistoryEntry),
    /// Reading failed. `source` contains the bytes consumed for this record.
    Skipped {
        source: SourceRecord,
        reason: ParseError,
    },
}

impl ParseOutcome {
    /// Convenience: `Some(&entry)` for `Entry`, `None` otherwise.
    pub fn entry(&self) -> Option<&HistoryEntry> {
        match self {
            ParseOutcome::Entry(e) => Some(e),
            ParseOutcome::Skipped { .. } => None,
        }
    }
}
