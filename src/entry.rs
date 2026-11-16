use std::{borrow::Cow, ops::Range, str::Utf8Error};

/// Original representation of a logical record in the parsed stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    /// Half-open byte range, including headers and physical line terminators.
    pub span: Range<u64>,
    /// Exact source bytes consumed within `span`.
    pub raw: Vec<u8>,
}

impl SourceRecord {
    pub(crate) fn empty(offset: u64) -> Self {
        Self {
            span: offset..offset,
            raw: Vec::new(),
        }
    }
}

/// Effective start time and elapsed seconds in the imported history list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Whether the file supplied a nonzero start time.
    pub from_file: bool,
    /// Start time in seconds since the Unix epoch.
    pub timestamp: i64,
    /// Command duration in seconds.
    pub duration: i64,
}

/// One parsed history entry.
///
/// Parsed entries carry the effective Zsh time, including its read-time fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HistoryEntry {
    /// Command bytes after shell-specific unescaping and multiline joining.
    /// No UTF-8 conversion or newline normalization is applied.
    pub command: Vec<u8>,

    /// Effective start time and duration. Present for every parsed entry.
    pub timing: Timing,

    /// Complete original record and its offsets in the input stream.
    pub source: SourceRecord,
}

impl HistoryEntry {
    /// Validate the command as UTF-8 without copying or replacing bytes.
    pub fn command_str(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(&self.command)
    }

    /// Display-only UTF-8 conversion. Use `command` or `source` for filtering
    /// and copying data without loss.
    pub fn command_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.command)
    }
}
