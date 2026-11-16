//! Logical records as consumed by Zsh 5.9's readhistline.
use crate::{ParseError, SourceRecord};
use std::io::{self, BufRead};

pub(crate) struct Records<R> {
    reader: R,
    offset: u64,
    // Zsh keeps the largest fgets buffer for subsequent records.
    width: usize,
    done: bool,
}

#[derive(Default)]
struct Segment {
    count: usize,
    nul: Option<usize>,
    last_body_byte: Option<u8>,
    newline: bool,
    eof: bool,
}

fn newline(bytes: &[u8]) -> Option<usize> {
    cfg_select! {
        feature = "memchr" => memchr::memchr(b'\n', bytes),
        _ => bytes.iter().position(|&b| b == b'\n'),
    }
}

impl<R: BufRead> Records<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            offset: 0,
            width: 1024,
            done: false,
        }
    }

    fn read_segment(&mut self, room: usize, source: &mut SourceRecord) -> io::Result<Segment> {
        let mut part = Segment::default();
        while part.count < room {
            let buf = match self.reader.fill_buf() {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if buf.is_empty() {
                part.eof = true;
                break;
            }
            let chunk = &buf[..buf.len().min(room - part.count)];
            let lf = newline(chunk);
            let body = &chunk[..lf.unwrap_or(chunk.len())];
            if part.nul.is_none() {
                part.nul = body.iter().position(|&b| b == 0).map(|i| part.count + i);
            }
            if let Some(&b) = body.last() {
                part.last_body_byte = Some(b);
            }
            let n = body.len() + usize::from(lf.is_some());
            source.raw.extend_from_slice(&chunk[..n]);
            self.reader.consume(n);
            self.offset += n as u64;
            source.span.end = self.offset;
            part.count += n;
            if lf.is_some() {
                part.newline = true;
                break;
            }
        }
        Ok(part)
    }
}

impl<R: BufRead> Iterator for Records<R> {
    type Item = (SourceRecord, Option<ParseError>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let mut source = SourceRecord::empty(self.offset);
        let mut start = 0;
        let mut last = None;
        loop {
            let part = match self.read_segment(self.width - start - 1, &mut source) {
                Ok(part) => part,
                Err(e) => {
                    self.done = true;
                    return Some((source, Some(e.into())));
                }
            };
            self.done = part.eof;
            if part.count == 0 {
                return None;
            }
            if part.nul.is_some_and(|nul| nul == 0 || !part.eof) {
                self.done = true;
                return Some((source, Some(ParseError::CorruptHistory)));
            }
            last = part.last_body_byte.or(last);
            start += part.count;
            if part.newline && last == Some(b'\\') {
                // The stored backslash + LF occupies one byte in the Zsh buffer.
                start -= 1;
                last = Some(b'\n');
            } else if part.newline || part.eof {
                return Some((source, None));
            } else {
                self.width = self.width.saturating_mul(2);
            }
        }
    }
}
