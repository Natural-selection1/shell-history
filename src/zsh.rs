//! Zsh plain and extended history. Decode commands without losing bytes;
//! keep original records separately for filtering and copying.
use crate::{HistoryEntry, ParseOutcome, Timing, lines::Records};
use std::{
    borrow::Cow,
    io::BufRead,
    time::{SystemTime, UNIX_EPOCH},
};

const META: u8 = 0x83;

/// Reverse Zsh metafy encoding, including a literal trailing marker.
pub fn unmetafy(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&META) {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter().copied();
    while let Some(b) = iter.next() {
        out.push(if b == META {
            iter.next().map_or(META, |next| next ^ 0x20)
        } else {
            b
        });
    }
    Cow::Owned(out)
}

#[derive(Debug, Clone, Default)]
pub struct ZshParser;

impl ZshParser {
    pub fn new() -> Self {
        Self
    }

    /// Stream plain and extended Zsh history records from a buffered reader.
    pub fn parse<R: BufRead>(&self, reader: R) -> impl Iterator<Item = ParseOutcome> {
        let read_time = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(t) => t.as_secs() as i64,
            Err(e) => {
                -(e.duration().as_secs() as i64) - i64::from(e.duration().subsec_nanos() != 0)
            }
        };
        Records::new(reader).map(move |(source, error)| {
            if let Some(reason) = error {
                return ParseOutcome::Skipped { source, reason };
            }
            let (command, timing) = decode(&source.raw, read_time);
            ParseOutcome::Entry(HistoryEntry {
                command,
                timing,
                source,
            })
        })
    }

    /// Convenience: parse, discarding skipped records and collecting entries.
    pub fn parse_vec(&self, reader: impl BufRead) -> Vec<HistoryEntry> {
        self.parse(reader)
            .filter_map(|o| match o {
                ParseOutcome::Entry(e) => Some(e),
                ParseOutcome::Skipped { .. } => None,
            })
            .collect()
    }
}

fn decode(raw: &[u8], read_time: i64) -> (Vec<u8>, Timing) {
    let raw = raw.split(|&b| b == 0).next().unwrap_or_default();
    let mut logical = Vec::with_capacity(raw.len());
    for physical in raw.split_inclusive(|&b| b == b'\n') {
        if let Some(body) = physical.strip_suffix(b"\n") {
            if let Some(prefix) = body.strip_suffix(b"\\") {
                logical.extend_from_slice(prefix);
                logical.push(b'\n');
            } else {
                // Zsh adds one space if the command ends with a backslash,
                // optionally followed by any number of literal spaces.
                let last_non_space = body.iter().rposition(|&b| b != b' ');
                let padding =
                    body.ends_with(b" ") && last_non_space.is_some_and(|p| body[p] == b'\\');
                logical.extend_from_slice(if padding {
                    &body[..body.len() - 1]
                } else {
                    body
                });
            }
        } else {
            logical.extend_from_slice(physical);
        }
    }
    let encoded = normalize_metafy(&logical);
    let (start, end, bytes) = header(&encoded);
    let from_file = start != 0;
    let timing = if from_file {
        let finish = if end < start {
            start.wrapping_add(end)
        } else {
            end
        };
        let duration = if finish == 0 {
            0
        } else {
            finish.wrapping_sub(start)
        };
        Timing {
            timestamp: start,
            duration,
            from_file,
        }
    } else {
        Timing {
            timestamp: read_time,
            duration: 0,
            from_file,
        }
    };
    (unmetafy(bytes).into_owned(), timing)
}

fn normalize_metafy(bytes: &[u8]) -> Cow<'_, [u8]> {
    let mut iter = bytes.iter();
    while let Some(&byte) = iter.next() {
        if byte == META && iter.next().is_some() {
            continue;
        }
        if (META..=0xa2).contains(&byte) {
            let mut normalized = Vec::with_capacity(bytes.len());
            for &b in unmetafy(bytes).iter() {
                if b == 0 || (META..=0xa2).contains(&b) {
                    normalized.extend_from_slice(&[META, b ^ 0x20]);
                } else {
                    normalized.push(b);
                }
            }
            return Cow::Owned(normalized);
        }
    }
    Cow::Borrowed(bytes)
}

fn header(line: &[u8]) -> (i64, i64, &[u8]) {
    let Some(fields) = line.strip_prefix(b":") else {
        let command = if line.starts_with(b"\\:") {
            &line[1..]
        } else {
            line
        };
        return (0, 0, command);
    };
    let start = number(fields);
    let Some(colon) = fields.iter().position(|&b| b == b':') else {
        return (start, start, &[]);
    };
    let rest = &fields[colon + 1..];
    let end = number(rest);
    let command = rest
        .iter()
        .position(|&b| b == b';')
        .map_or(&[][..], |i| &rest[i + 1..]);
    (start, end, command)
}

// zstrtol(s, NULL, 0) as used by the 64-bit Zsh history reader.
fn number(mut bytes: &[u8]) -> i64 {
    while let [b' ' | b'\t' | b'\n', rest @ ..] = bytes {
        bytes = rest;
    }
    let negative = bytes.starts_with(b"-");
    if negative || bytes.starts_with(b"+") {
        bytes = &bytes[1..];
    }
    let (base, digits) = match bytes {
        [b'0', b'x' | b'X', rest @ ..] => (16, rest),
        [b'0', b'b' | b'B', rest @ ..] => (2, rest),
        [b'0', rest @ ..] => (8, rest),
        rest => (10, rest),
    };
    let mut value = 0u64;
    let mut truncated = false;
    for &b in digits {
        let Some(digit) = char::from(b).to_digit(base) else {
            break;
        };
        let next = value
            .wrapping_mul(u64::from(base))
            .wrapping_add(u64::from(digit));
        if next < value {
            truncated = true;
            break;
        }
        value = next;
    }
    if !truncated && value > i64::MAX as u64 && !(negative && value == 1 << 63) {
        value /= u64::from(base);
    }
    if negative {
        (value as i64).wrapping_neg()
    } else {
        value as i64
    }
}
